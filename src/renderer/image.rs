use bevy::log;
use std::path::PathBuf;

use ash::vk;
use color_eyre::eyre::Result;
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator},
    MemoryLocation,
};

use super::{
    buffer::AllocatedBuffer, upload_context::UploadContext, vkinit, vkutils,
    ASSETS_DIR,
};

pub struct AllocatedImageCreateInfo {
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub usage_flags: vk::ImageUsageFlags,
    pub aspect_flags: vk::ImageAspectFlags,
    pub name: String,
    pub desc_set: Option<vk::DescriptorSet>,
}

pub struct AllocatedImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub aspect: vk::ImageAspectFlags,
    pub allocation: Allocation,
    pub desc_set: Option<vk::DescriptorSet>, // Some if accessible from shaders, None otherwise
}

impl AllocatedImage {
    pub fn new(
        create_info: &AllocatedImageCreateInfo,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let image = {
            let info = vkinit::image_create_info(
                create_info.format,
                create_info.usage_flags,
                create_info.extent,
            );
            unsafe { device.create_image(&info, None)? }
        };
        let reqs = unsafe { device.get_image_memory_requirements(image) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: &create_info.name,
            requirements: reqs,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::DedicatedImage(image),
        })?;
        unsafe {
            device.bind_image_memory(image, allocation.memory(), 0)?;
        }
        let view = {
            let info = vkinit::image_view_create_info(
                create_info.format,
                image,
                create_info.aspect_flags,
            );
            unsafe { device.create_image_view(&info, None)? }
        };

        Ok(Self {
            image,
            view,
            format: create_info.format,
            extent: create_info.extent,
            aspect: create_info.aspect_flags,
            allocation,
            desc_set: create_info.desc_set,
        })
    }

    pub fn new_depth_image(
        width: u32,
        height: u32,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let create_info = AllocatedImageCreateInfo {
            format: vk::Format::D32_SFLOAT,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            usage_flags: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            aspect_flags: vk::ImageAspectFlags::DEPTH,
            name: "Depth image".into(),
            desc_set: None,
        };
        let image = Self::new(&create_info, device, allocator)?;

        Ok(image)
    }

    pub fn load_from_file(
        filename: &str,
        flipv: bool,
        device: &ash::Device,
        allocator: &mut Allocator,
        upload_context: &UploadContext,
        desc_set: Option<vk::DescriptorSet>,
    ) -> Result<Self> {
        let (staging_buffer, img_width, img_height) = {
            let filepath = unsafe {
                let mut path = PathBuf::from(ASSETS_DIR.clone().unwrap());
                path.push(filename);
                path
            };
            let img = image::open(filepath)?.into_rgba8();
            let mut img = image::DynamicImage::ImageRgba8(img);
            if flipv {
                img = img.flipv();
            }
            let img_width = img.width();
            let img_height = img.height();
            let data = img.as_bytes();

            // Each component of rgba is 1 byte
            // Multiply by 4 because there are 4 components (r, g, b, a)
            let img_size = img_width * img_height * 4;
            let mut staging_buffer = AllocatedBuffer::new(
                device,
                allocator,
                img_size as u64,
                vk::BufferUsageFlags::TRANSFER_SRC,
                "Image staging buffer",
                gpu_allocator::MemoryLocation::CpuToGpu,
            )?;
            let _ = staging_buffer.write(data, 0);

            (staging_buffer, img_width, img_height)
        };

        let img = {
            let img_extent = vk::Extent3D {
                width: img_width,
                height: img_height,
                depth: 1,
            };

            let create_info = AllocatedImageCreateInfo {
                format: vk::Format::R8G8B8A8_SRGB,
                extent: img_extent,
                usage_flags: vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_DST,
                aspect_flags: vk::ImageAspectFlags::COLOR,
                name: format!("Image from file: {}", filename),
                desc_set,
            };

            Self::new(&create_info, device, allocator)?
        };

        let _ = upload_context.immediate_submit(
            |cmd: &vk::CommandBuffer, device: &ash::Device| {
                let range = vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                };

                let img_barrier_to_transfer = vk::ImageMemoryBarrier {
                    old_layout: vk::ImageLayout::UNDEFINED,
                    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    image: img.image,
                    subresource_range: range,
                    src_access_mask: vk::AccessFlags::empty(),
                    dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    ..Default::default()
                };

                unsafe {
                    // Create a pipeline barrier that blocks from
                    // VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT to VK_PIPELINE_STAGE_TRANSFER_BIT
                    // Read more: https://gpuopen.com/learn/vulkan-barriers-explained/
                    device.cmd_pipeline_barrier(
                        *cmd,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[img_barrier_to_transfer],
                    );
                }

                let copy_region = vk::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    image_extent: img.extent,
                    ..Default::default()
                };

                unsafe {
                    // Copy staging buffer into image
                    device.cmd_copy_buffer_to_image(
                        *cmd,
                        staging_buffer.buffer,
                        img.image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[copy_region],
                    );
                }

                let mut img_barrier_to_readable = img_barrier_to_transfer;
                img_barrier_to_readable.old_layout =
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL;
                img_barrier_to_readable.new_layout =
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                img_barrier_to_readable.src_access_mask =
                    vk::AccessFlags::TRANSFER_WRITE;
                img_barrier_to_readable.dst_access_mask =
                    vk::AccessFlags::SHADER_READ;

                // Barrier the image into the shader-readable layout
                unsafe {
                    device.cmd_pipeline_barrier(
                        *cmd,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[img_barrier_to_readable],
                    )
                }
            },
            device,
        );

        staging_buffer.cleanup(device, allocator);

        Ok(img)
    }

    pub fn transition_layout(
        &mut self,
        cmd: vk::CommandBuffer,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        device: &ash::Device,
    ) {
        vkutils::transition_image_layout(
            cmd,
            self.image,
            self.aspect,
            old_layout,
            new_layout,
            device,
        );
    }

    pub fn copy_to_image(
        &self,
        cmd: vk::CommandBuffer,
        dst_image: vk::Image,
        dst_image_extent: vk::Extent2D,
        device: &ash::Device,
    ) {
        vkutils::copy_image_to_image(
            cmd,
            self.image,
            dst_image,
            vk::Extent2D {
                width: self.extent.width,
                height: self.extent.height,
            },
            dst_image_extent,
            device,
        );
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            device.destroy_image_view(self.view, None);
            allocator.free(self.allocation).unwrap();
            device.destroy_image(self.image, None);
        }
    }
}

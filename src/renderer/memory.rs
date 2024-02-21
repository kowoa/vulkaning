use std::{
    path::PathBuf,
    sync::{Arc, MutexGuard},
};

use ash::vk;
use color_eyre::eyre::{OptionExt, Result};
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator},
    MemoryLocation,
};

use super::{
    resources::model::ASSETS_DIR, upload_context::UploadContext, vkinit,
};

#[derive(Debug)]
pub struct AllocatedBuffer {
    pub buffer: vk::Buffer,
    pub allocation: Allocation,
    pub size: u64,
    pub offsets: Option<Vec<u32>>,
}

impl AllocatedBuffer {
    pub fn new(
        device: &ash::Device,
        allocator: &mut Allocator,
        buffer_size: u64,
        buffer_usage: vk::BufferUsageFlags,
        alloc_name: &str,
        alloc_loc: MemoryLocation,
    ) -> Result<Self> {
        let buffer = {
            let buffer_info = vk::BufferCreateInfo {
                size: buffer_size,
                usage: buffer_usage,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                ..Default::default()
            };
            unsafe { device.create_buffer(&buffer_info, None)? }
        };

        let reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: alloc_name,
            requirements: reqs,
            location: alloc_loc,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        unsafe {
            device.bind_buffer_memory(
                buffer,
                allocation.memory(),
                allocation.offset(),
            )?;
        }

        Ok(Self {
            buffer,
            allocation,
            size: buffer_size,
            offsets: None,
        })
    }

    pub fn set_offsets(&mut self, offsets: Vec<u32>) {
        self.offsets = Some(offsets);
    }

    pub fn write<T>(
        &mut self,
        data: &[T],
        start_offset: usize,
    ) -> Result<presser::CopyRecord>
    where
        T: Copy,
    {
        Ok(presser::copy_from_slice_to_offset(
            data,
            &mut self.allocation,
            start_offset,
        )?)
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            allocator.free(self.allocation).unwrap();
            device.destroy_buffer(self.buffer, None);
        }
    }
}

pub struct AllocatedImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub allocation: Allocation,
}

impl AllocatedImage {
    pub fn new(
        format: vk::Format,
        extent: vk::Extent3D,
        usage_flags: vk::ImageUsageFlags,
        aspect_flags: vk::ImageAspectFlags,
        name: &str,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let image = {
            let info = vkinit::image_create_info(format, usage_flags, extent);
            unsafe { device.create_image(&info, None)? }
        };
        let reqs = unsafe { device.get_image_memory_requirements(image) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name,
            requirements: reqs,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::DedicatedImage(image),
        })?;
        unsafe {
            device.bind_image_memory(image, allocation.memory(), 0)?;
        }
        let image_view = {
            let info =
                vkinit::image_view_create_info(format, image, aspect_flags);
            unsafe { device.create_image_view(&info, None)? }
        };

        Ok(Self {
            image,
            view: image_view,
            format,
            extent,
            allocation,
        })
    }

    pub fn new_depth_image(
        extent: vk::Extent3D,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let image = Self::new(
            vk::Format::D32_SFLOAT,
            extent,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
            "Depth image",
            device,
            allocator,
        )?;

        Ok(image)
    }

    pub fn load_from_file(
        filename: &str,
        device: &ash::Device,
        allocator: &mut Allocator,
        upload_context: &UploadContext,
    ) -> Result<Self> {
        let (staging_buffer, img_width, img_height) = {
            let filepath = unsafe {
                let mut path = PathBuf::from(ASSETS_DIR.clone().unwrap());
                path.push(filename);
                path
            };
            let img = image::open(filepath)?;
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
            
            Self::new(
                vk::Format::R8G8B8A8_SRGB,
                img_extent,
                vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_DST,
                vk::ImageAspectFlags::COLOR,
                "Image from file",
                device,
                allocator,
            )?
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

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            device.destroy_image_view(self.view, None);
            allocator.free(self.allocation).unwrap();
            device.destroy_image(self.image, None);
        }
    }
}

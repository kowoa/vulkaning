use crate::renderer::{
    image::AllocatedImage, upload_context::UploadContext, vkinit,
};
use ash::vk;
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;

pub struct Texture {
    pub image: AllocatedImage,
    pub sampler: vk::Sampler,
    pub desc_set: vk::DescriptorSet,
}

impl Texture {
    pub fn load_from_file(
        filename: &str,
        descriptor_pool: &vk::DescriptorPool,
        single_texture_desc_set_layout: &vk::DescriptorSetLayout,
        device: &ash::Device,
        allocator: &mut Allocator,
        upload_context: &UploadContext,
    ) -> Result<Self> {
        let image = AllocatedImage::load_from_file(
            filename,
            device,
            allocator,
            upload_context,
        )?;

        let sampler = {
            // NEAREST makes texture look blocky
            let info = vkinit::sampler_create_info(
                vk::Filter::NEAREST,
                vk::SamplerAddressMode::REPEAT,
            );
            unsafe { device.create_sampler(&info, None)? }
        };

        let desc_set = {
            let info = vk::DescriptorSetAllocateInfo {
                descriptor_pool: *descriptor_pool,
                descriptor_set_count: 1,
                p_set_layouts: single_texture_desc_set_layout,
                ..Default::default()
            };
            unsafe { device.allocate_descriptor_sets(&info)? }
        }[0];

        let info = vk::DescriptorImageInfo {
            sampler,
            image_view: image.view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        };
        let write = vkinit::write_descriptor_image(
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            desc_set,
            0,
            &info,
        );

        unsafe { device.update_descriptor_sets(&[write], &[]) }

        Ok(Self {
            image,
            sampler,
            desc_set,
        })
    }
}

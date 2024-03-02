use bevy::log;
use crate::renderer::{
    descriptors::DescriptorAllocator, image::AllocatedImage,
    upload_context::UploadContext, vkinit,
};
use ash::vk;
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;

pub struct Texture {
    image: AllocatedImage,
    sampler: vk::Sampler,
}

impl Texture {
    pub fn load_from_file(
        filename: &str,
        flipv: bool,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
        upload_context: &UploadContext,
    ) -> Result<Self> {
        // Allocate new descriptor set
        let desc_set = desc_allocator.allocate(device, "single texture")?;

        let image = AllocatedImage::load_from_file(
            filename,
            flipv,
            device,
            allocator,
            upload_context,
            Some(desc_set),
        )?;

        let sampler = {
            // NEAREST makes texture look blocky
            let info = vkinit::sampler_create_info(
                vk::Filter::NEAREST,
                vk::SamplerAddressMode::REPEAT,
            );
            unsafe { device.create_sampler(&info, None)? }
        };

        // Update new descriptor set
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

        Ok(Self { image, sampler })
    }

    pub fn desc_set(&self) -> vk::DescriptorSet {
        self.image.desc_set.unwrap()
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        self.image.cleanup(device, allocator);
        unsafe {
            device.destroy_sampler(self.sampler, None);
        }
    }
}

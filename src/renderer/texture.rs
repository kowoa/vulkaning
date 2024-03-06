use crate::renderer::{
    descriptors::DescriptorAllocator, image::AllocatedImage,
    upload_context::UploadContext, vkinit,
};
use ash::vk;
use bevy::log;
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;

/// A texture is an image with a sampler and descriptor set
pub struct Texture {
    data: Vec<u8>,
    width: u32,
    height: u32,
    image: Option<AllocatedImage>,
    sampler: Option<vk::Sampler>,
    desc_set: Option<vk::DescriptorSet>,
}

impl Texture {
    pub fn new_uninitialized(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
            image: None,
            sampler: None,
            desc_set: None,
        }
    }

    pub fn initialize_and_upload(
        &mut self,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
        upload_context: &UploadContext,
    ) -> Result<()> {
        if self.data.is_empty() {
            return Err(eyre!(
                "Texture has no data or has already been initialized"
            ));
        }

        if self.image.is_none() {
            let image = AllocatedImage::from_bytes(
                &self.data,
                self.width,
                self.height,
            )?;
            self.image = Some(image);
        }
        if self.sampler.is_none() {
            self.sampler = Some(Self::default_sampler(device));
        }
        if self.desc_set.is_none() {
            self.desc_set =
                Some(desc_allocator.allocate(device, "single texture")?);
        }
        Ok(())
    }

    pub fn new(
        image: AllocatedImage,
        sampler: vk::Sampler,
        desc_set: vk::DescriptorSet,
        device: &ash::Device,
    ) -> Result<Self> {
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

        Ok(Self {
            data: Vec::new(),
            width: image.extent.width,
            height: image.extent.height,
            image: Some(image),
            sampler: Some(sampler),
            desc_set: Some(desc_set),
        })
    }

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
        )?;

        let sampler = {
            // NEAREST makes texture look blocky
            let info = vkinit::sampler_create_info(
                vk::Filter::NEAREST,
                vk::SamplerAddressMode::REPEAT,
            );
            unsafe { device.create_sampler(&info, None)? }
        };

        Self::new(image, sampler, desc_set, device)
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

    fn default_sampler(device: &ash::Device) -> vk::Sampler {
        // NEAREST makes texture look blocky
        let info = vkinit::sampler_create_info(
            vk::Filter::NEAREST,
            vk::SamplerAddressMode::REPEAT,
        );
        unsafe { device.create_sampler(&info, None)? }
    }
}

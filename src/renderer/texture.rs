use crate::renderer::{
    descriptors::DescriptorAllocator, image::AllocatedImage,
    upload_context::UploadContext, vkinit,
};
use ash::vk;
use bevy::{asset::Asset, reflect::TypePath};
use color_eyre::eyre::{eyre, OptionExt, Result};
use gpu_allocator::vulkan::Allocator;
use image::{ImageBuffer, Rgba};

/// A texture is an image with a sampler and descriptor set
#[derive(Asset, TypePath)]
pub struct Texture {
    data: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    image: Option<AllocatedImage>,
    sampler: Option<vk::Sampler>,
}

impl Texture {
    pub fn new_compute_texture(
        width: u32,
        height: u32,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<Self> {
        let image = AllocatedImage::new_storage_image(
            width,
            height,
            desc_allocator.allocate(device, "compute texture")?,
            device,
            allocator,
        )?;
        Ok(Self {
            data: None,
            image: Some(image),
            sampler: None,
        })
    }

    pub fn new_graphics_texture(
        data: ImageBuffer<Rgba<u8>, Vec<u8>>,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
        upload_context: &UploadContext,
        flipv: bool,
    ) -> Result<Self> {
        let mut image = Self::new_uninitialized(data);
        image.init_graphics_texture(
            device,
            allocator,
            desc_allocator,
            upload_context,
            flipv,
        )?;
        Ok(image)
    }

    pub fn new_uninitialized(data: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Self {
        Self {
            data: Some(data),
            image: None,
            sampler: None,
        }
    }

    pub fn init_graphics_texture(
        &mut self,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
        upload_context: &UploadContext,
        flipv: bool,
    ) -> Result<()> {
        if self.image.is_some() || self.sampler.is_some() {
            return Err(eyre!("Cannot initialize graphics texture because texture is already initialized"));
        }
        let data = self.data.take().ok_or_eyre("Texture data is empty")?;
        let mut img = image::DynamicImage::ImageRgba8(data);
        if flipv {
            img = img.flipv();
        }
        let width = img.width();
        let height = img.height();
        let data = img.as_bytes();

        self.sampler = Some(Self::default_sampler(device)?);
        self.image = Some(AllocatedImage::new_color_image(
            data,
            width,
            height,
            desc_allocator.allocate(device, "graphics texture")?,
            self.sampler.unwrap(),
            device,
            allocator,
            upload_context,
        )?);

        Ok(())
    }

    pub fn load_from_file(
        filename: &str,
        flipv: bool,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
        upload_context: &UploadContext,
    ) -> Result<Self> {
        let desc_set = desc_allocator.allocate(device, "graphics texture")?;
        let sampler = Self::default_sampler(device)?;
        let image = AllocatedImage::load_from_file(
            filename,
            flipv,
            desc_set,
            sampler,
            device,
            allocator,
            upload_context,
        )?;

        Ok(Self {
            data: None,
            image: Some(image),
            sampler: Some(sampler),
        })
    }

    pub fn image(&self) -> &AllocatedImage {
        self.image.as_ref().unwrap()
    }

    pub fn image_mut(&mut self) -> &mut AllocatedImage {
        self.image.as_mut().unwrap()
    }

    pub fn desc_set(&self) -> vk::DescriptorSet {
        self.image.as_ref().unwrap().desc_set.unwrap()
    }

    pub fn width(&self) -> u32 {
        self.image.as_ref().unwrap().extent.width
    }

    pub fn height(&self) -> u32 {
        self.image.as_ref().unwrap().extent.height
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        self.image.unwrap().cleanup(device, allocator);
        if let Some(sampler) = self.sampler {
            unsafe {
                device.destroy_sampler(sampler, None);
            }
        }
    }

    fn default_sampler(device: &ash::Device) -> Result<vk::Sampler> {
        // NEAREST makes texture look blocky
        let info = vkinit::sampler_create_info(
            vk::Filter::NEAREST,
            vk::SamplerAddressMode::REPEAT,
        );
        Ok(unsafe { device.create_sampler(&info, None)? })
    }
}

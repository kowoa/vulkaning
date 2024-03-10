use crate::renderer::image::AllocatedImage;
use ash::vk;
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;
use image::{ImageBuffer, Rgba};

use super::context::Context;

/// Asset data sent from the asset loader
pub struct TextureAssetData {
    pub data: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    pub flipv: bool,
    pub filter: vk::Filter,
}

impl Default for TextureAssetData {
    fn default() -> Self {
        Self {
            data: None,
            flipv: false,
            filter: vk::Filter::NEAREST,
        }
    }
}

/// A texture is an image with a sampler and descriptor set
#[derive(Debug)]
pub struct Texture {
    image: AllocatedImage,
    sampler: Option<vk::Sampler>, // Only Some if texture is not used for compute
}

impl Texture {
    pub fn new_compute_texture(
        width: u32,
        height: u32,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let image = AllocatedImage::new_storage_image(
            width, height, device, allocator,
        )?;
        Ok(Self {
            image,
            sampler: None,
        })
    }

    pub fn new_graphics_texture(
        asset: TextureAssetData,
        sampler: vk::Sampler,
        ctx: &Context,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let image_data = asset.data.unwrap();
        let width = image_data.width();
        let height = image_data.height();
        let data = if asset.flipv {
            let mut img = image::DynamicImage::ImageRgba8(image_data);
            img = img.flipv();
            img.into_bytes()
        } else {
            image_data.into_raw()
        };

        let image = AllocatedImage::new_color_image(
            &data, width, height, ctx, allocator,
        )?;

        Ok(Self {
            image,
            sampler: Some(sampler),
        })
    }

    pub fn image(&self) -> &AllocatedImage {
        &self.image
    }

    pub fn image_mut(&mut self) -> &mut AllocatedImage {
        &mut self.image
    }

    pub fn sampler(&self) -> Option<vk::Sampler> {
        self.sampler
    }

    pub fn width(&self) -> u32 {
        self.image.extent.width
    }

    pub fn height(&self) -> u32 {
        self.image.extent.height
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        self.image.cleanup(device, allocator);
    }
}

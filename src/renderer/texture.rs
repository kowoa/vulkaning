use crate::renderer::{image::AllocatedImage, upload_context::UploadContext};
use ash::vk;
use bevy::{asset::Asset, reflect::TypePath};
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;
use image::{ImageBuffer, Rgba};

/// Asset data sent from the asset loader
pub struct TextureAssetData {
    pub data: ImageBuffer<Rgba<u8>, Vec<u8>>,
    pub flipv: bool,
    pub filter: vk::Filter,
}

/// A texture is an image with a sampler and descriptor set
#[derive(Asset, TypePath, Debug)]
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
        data: TextureAssetData,
        sampler: vk::Sampler,
        device: &ash::Device,
        allocator: &mut Allocator,
        upload_context: &UploadContext,
    ) -> Result<Self> {
        let width = data.data.width();
        let height = data.data.height();
        let data = if data.flipv {
            let mut img = image::DynamicImage::ImageRgba8(data.data);
            img = img.flipv();
            img.into_bytes()
        } else {
            data.data.into_raw()
        };

        let image = AllocatedImage::new_color_image(
            &data,
            width,
            height,
            device,
            allocator,
            upload_context,
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

    pub fn width(&self) -> u32 {
        self.image.extent.width
    }

    pub fn height(&self) -> u32 {
        self.image.extent.height
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        self.image.cleanup(device, allocator);
        if let Some(sampler) = self.sampler {
            unsafe {
                device.destroy_sampler(sampler, None);
            }
        }
    }
}

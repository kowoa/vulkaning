use std::collections::HashMap;

use ash::vk;
use color_eyre::eyre::{eyre, Result};
use gpu_allocator::vulkan::Allocator;

use super::{material::Material, model::Model, texture::Texture, vkinit};

/// Shared resources for rendering
#[derive(Default)]
pub struct RenderResources {
    pub models: HashMap<String, Model>,
    pub textures: HashMap<String, Texture>,
    pub materials: HashMap<String, Material>,
    pub samplers: HashMap<vk::Filter, vk::Sampler>,
    pub desc_set_layouts: HashMap<String, vk::DescriptorSetLayout>,
}

impl RenderResources {
    pub fn create_sampler(
        &mut self,
        filter: vk::Filter,
        device: &ash::Device,
    ) -> Result<()> {
        if self.samplers.contains_key(&filter) {
            return Err(eyre!("Sampler already exists"));
        }
        let sampler_info =
            vkinit::sampler_create_info(filter, vk::SamplerAddressMode::REPEAT);
        let sampler = unsafe { device.create_sampler(&sampler_info, None)? };
        self.samplers.insert(filter, sampler);
        Ok(())
    }

    pub fn cleanup(&mut self, device: &ash::Device, allocator: &mut Allocator) {
        self.models
            .drain()
            .for_each(|(_, model)| model.cleanup(device, allocator));
        self.textures
            .drain()
            .for_each(|(_, texture)| texture.cleanup(device, allocator));
        self.materials
            .drain()
            .for_each(|(_, material)| material.cleanup(device));
        self.samplers.drain().for_each(|(_, sampler)| unsafe {
            device.destroy_sampler(sampler, None);
        });
        self.desc_set_layouts
            .drain()
            .for_each(|(_, layout)| unsafe {
                device.destroy_descriptor_set_layout(layout, None)
            });
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

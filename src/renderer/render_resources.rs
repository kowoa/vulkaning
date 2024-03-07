use std::collections::HashMap;

use bevy::ecs::system::Resource;
use gpu_allocator::vulkan::Allocator;

use super::{material::Material, model::Model, texture::Texture};

#[derive(Resource, Default)]
pub struct RenderResources {
    pub models: HashMap<String, Model>,
    pub textures: HashMap<String, Texture>,
    pub materials: HashMap<String, Material>,
}

impl RenderResources {
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
    }
}

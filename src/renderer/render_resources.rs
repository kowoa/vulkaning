use std::collections::HashMap;

use bevy::ecs::system::Resource;

use super::{model::Model, texture::Texture};

#[derive(Resource, Default)]
pub struct RenderResources {
    pub models: HashMap<String, Model>,
    pub textures: HashMap<String, Texture>,
}

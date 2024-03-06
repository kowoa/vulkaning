use std::collections::HashMap;

use bevy::ecs::system::Resource;

use super::model::Model;

#[derive(Resource, Default)]
pub struct RenderResources {
    pub models: HashMap<String, Model>,
}

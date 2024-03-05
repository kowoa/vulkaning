mod obj;

use bevy::asset::RecursiveDependencyLoadState;
use color_eyre::eyre::Result;

use bevy::prelude::*;

use crate::renderer::{mesh::Mesh, model::Model, vertex::Vertex};

use self::obj::ObjAssetsLoading;

pub struct AssetsPlugin;
impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(obj::ObjAssetsPlugin)
            .add_systems(Startup, load_obj_models)
            .add_systems(Update, check_obj_models);
    }
}

fn load_obj_models(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ObjAssetsLoading>,
) {
    let monkey_handle: Handle<Model> = asset_server.load("monkey_smooth.obj");
    loading.0.insert("monkey".into(), monkey_handle.untyped());
}

fn check_obj_models(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ObjAssetsLoading>,
    models: Res<Assets<Model>>,
) {
    let mut loaded_models = Vec::new();
    for (name, handle) in loading.0.iter() {
        let state = asset_server.recursive_dependency_load_state(handle);
        if state == RecursiveDependencyLoadState::Loaded {
            bevy::log::error!("{} is loaded", name);
            let model = models.get(handle.clone().typed()).unwrap();
            bevy::log::error!("Model: {:?}", model);
        }
        loaded_models.push(name);
    }
}

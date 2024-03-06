mod obj;

use bevy::asset::RecursiveDependencyLoadState;
use color_eyre::eyre::Result;

use bevy::prelude::*;

use crate::renderer::{mesh::Mesh, model::Model, vertex::Vertex};

pub use self::obj::ObjAssetsLoading;
pub use self::obj::ObjAssetsState;

pub struct AssetsPlugin;
impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(obj::ObjAssetsPlugin)
            .add_systems(Startup, load_obj_models);
    }
}

fn load_obj_models(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ObjAssetsLoading>,
) {
    let monkey_handle: Handle<Model> = asset_server.load("monkey_smooth.obj");
    loading.0.insert(
        "monkey".into(),
        (monkey_handle.untyped(), ObjAssetsState::NotLoaded),
    );
}

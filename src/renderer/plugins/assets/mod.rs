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
            .add_systems(Startup, load_obj_models)
            .add_systems(
                Update,
                check_obj_models.run_if(in_state(ObjAssetsState::NotLoaded)),
            );
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

fn check_obj_models(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ObjAssetsLoading>,
    mut state: ResMut<NextState<ObjAssetsState>>,
) {
    for (name, (handle, load_state)) in loading.0.iter_mut() {
        if *load_state == ObjAssetsState::Loaded {
            continue;
        }
        let state = asset_server.recursive_dependency_load_state(handle.id());
        if state == RecursiveDependencyLoadState::Loaded {
            *load_state = ObjAssetsState::Loaded;
        }
    }

    // If all models are loaded, change the state to Loaded
    if loading
        .0
        .values()
        .all(|(_, state)| *state == ObjAssetsState::Loaded)
    {
        state.set(ObjAssetsState::Loaded);
    }
}

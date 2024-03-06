use std::collections::HashMap;

use bevy::prelude::*;

use crate::renderer::texture::Texture;

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "png"];

#[derive(States, Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum ImageAssetsLoadState {
    NotLoaded,
    Loaded,
}

#[derive(Resource, Default)]
struct ImageAssetsLoading(pub HashMap<String, Handle<Texture>>);

pub struct ImageAssetsPlugin;
impl Plugin for ImageAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.preregister_asset_loader::<ImageLoader>(IMAGE_EXTENSIONS)
            .insert_state(ImageAssetsLoadState::NotLoaded) // Loaded when all obj assets get loaded
            .init_resource::<ImageAssetsLoading>()
            .add_systems(
                Update,
                check_all_image_assets_loaded
                    .run_if(in_state(ImageAssetsLoadState::NotLoaded)),
            );
    }

    fn finish(&self, app: &mut App) {
        /*
                app.register_asset_loader(ImageLoader);
        */
    }
}

/*
fn check_all_image_assets_loaded(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ImageAssetsLoading>,
    mut state: ResMut<NextState<ObjAssetsState>>,
) {
    for (name, (handle, load_state)) in loading.0.iter_mut() {
        if *load_state == ImageAssetsLoadState::Loaded {
            continue;
        }
        let state = asset_server.recursive_dependency_load_state(handle.id());
        if state == RecursiveDependencyLoadState::Loaded {
            *load_state = ImageAssetsLoadState::Loaded;
        }
    }

    // If all models are loaded, change the state to Loaded
    if loading
        .0
        .values()
        .all(|(_, state)| *state == ObjAssetsState::Loaded)
    {
        state.set(ImageAssetsLoadState::Loaded);
    }
}

struct ImageLoader;
*/

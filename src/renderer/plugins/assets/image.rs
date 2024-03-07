use std::collections::HashMap;

use bevy::{
    asset::{
        io::Reader, AssetLoader, AsyncReadExt, LoadContext,
        RecursiveDependencyLoadState,
    },
    prelude::*,
};
use bevy_utils::BoxedFuture;
use image::ImageError;

use crate::renderer::{render_resources::RenderResources, texture::Texture};

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
        app.register_asset_loader(ImageLoader);
    }
}

fn check_all_image_assets_loaded(
    asset_server: Res<AssetServer>,
    mut loading_textures: ResMut<ImageAssetsLoading>,
    mut loaded_textures: ResMut<Assets<Texture>>,
    mut state: ResMut<NextState<ImageAssetsLoadState>>,
    mut resources: ResMut<RenderResources>,
) {
    let mut to_remove = Vec::new();
    for (name, handle) in loading_textures.0.iter_mut() {
        // Check if model has fully loaded
        let state = asset_server.recursive_dependency_load_state(handle.id());
        if state == RecursiveDependencyLoadState::Loaded {
            to_remove.push(name.clone());
            // Insert model into render resources
            let texture = loaded_textures.remove(handle.clone_weak()).unwrap();
            resources.textures.insert(name.to_owned(), texture);
        }
    }

    for name in to_remove {
        loading_textures.0.remove(&name);
    }

    // If all models are loaded, change the state to Loaded
    if loading_textures.0.is_empty() {
        state.set(ImageAssetsLoadState::Loaded);
    }
}

struct ImageLoader;
impl AssetLoader for ImageLoader {
    type Error = ImageError;
    type Settings = ();
    type Asset = Texture;

    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            load_image(&bytes, load_context).await
        })
    }

    fn extensions(&self) -> &[&str] {
        IMAGE_EXTENSIONS
    }
}

async fn load_image<'a, 'b>(
    bytes: &'a [u8],
    _load_context: &'a mut LoadContext<'b>,
) -> Result<Texture, ImageError> {
    let image = image::load_from_memory(bytes)?.into_rgba8();
    Ok(Texture::new_uninitialized(image))
}

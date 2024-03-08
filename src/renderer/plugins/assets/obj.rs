use bevy::asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext};
use bevy::asset::{AssetPath, RecursiveDependencyLoadState};
use bevy::prelude::*;
use bevy_utils::BoxedFuture;
use color_eyre::eyre::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

use crate::renderer::mesh::Mesh;
use crate::renderer::model::Model;
use crate::renderer::render_resources::RenderResources;
use crate::renderer::vertex::Vertex;

const OBJ_EXTENSIONS: &[&str] = &["obj"];

#[derive(States, Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum ObjAssetsLoadState {
    NotLoaded,
    Loaded,
}

#[derive(Resource, Default)]
pub struct ObjAssetsLoading(pub HashMap<String, Handle<Model>>);

pub struct ObjAssetsPlugin;
impl Plugin for ObjAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.preregister_asset_loader::<ObjLoader>(OBJ_EXTENSIONS)
            .insert_state(ObjAssetsLoadState::NotLoaded) // Loaded when all obj assets get loaded
            .init_resource::<ObjAssetsLoading>()
            .add_systems(
                Update,
                check_all_obj_assets_loaded
                    .run_if(in_state(ObjAssetsLoadState::NotLoaded)),
            );
    }

    fn finish(&self, app: &mut App) {
        app.register_asset_loader(ObjLoader);
    }
}

fn check_all_obj_assets_loaded(
    asset_server: Res<AssetServer>,
    mut loading_models: ResMut<ObjAssetsLoading>,
    mut loaded_models: ResMut<Assets<Model>>,
    mut state: ResMut<NextState<ObjAssetsLoadState>>,
    mut resources: ResMut<RenderResources>,
) {
    let mut to_remove = Vec::new();
    for (name, handle) in loading_models.0.iter_mut() {
        // Check if model has fully loaded
        let state = asset_server.recursive_dependency_load_state(handle.id());
        if state == RecursiveDependencyLoadState::Loaded {
            to_remove.push(name.clone());
            // Insert model into render resources
            let model = loaded_models.remove(handle.clone_weak()).unwrap();
            resources.models.insert(name.to_owned(), model);
        }
    }

    for name in to_remove {
        loading_models.0.remove(&name);
    }

    // If all models are loaded, change the state to Loaded
    if loading_models.0.is_empty() {
        state.set(ObjAssetsLoadState::Loaded);
    }
}

struct ObjLoader;

impl AssetLoader for ObjLoader {
    type Error = ObjError;
    type Settings = ();
    type Asset = Model;

    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            load_obj(&bytes, load_context).await
        })
    }

    fn extensions(&self) -> &[&str] {
        OBJ_EXTENSIONS
    }
}

#[allow(clippy::derivable_impls)] // TODO remove?
impl Default for ObjLoader {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Error, Debug)]
pub enum ObjError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid OBJ file: {0}")]
    TobjError(#[from] tobj::LoadError),
    #[error("Failed to load materials for {0}: {1}")]
    MaterialError(PathBuf, #[source] tobj::LoadError),
    #[error("Invalid image file for texture: {0}")]
    InvalidImageFile(PathBuf),
    #[error("Asset reading failed: {0}")]
    AssetLoadError(#[from] bevy::asset::AssetLoadError),
}

async fn load_obj<'a, 'b>(
    bytes: &'a [u8],
    load_context: &'a mut LoadContext<'b>,
) -> Result<Model, ObjError> {
    load_obj_model(bytes, load_context).await
}

async fn load_obj_model<'a, 'b>(
    bytes: &'a [u8],
    load_context: &'a mut LoadContext<'b>,
) -> Result<Model, ObjError> {
    let (models, materials) = load_obj_data(bytes, load_context).await?;

    #[allow(unused_variables)]
    let materials = materials.map_err(|err| {
        let obj_path = load_context.path().to_path_buf();
        ObjError::MaterialError(obj_path, err)
    })?;

    let mut indices = Vec::new();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut texcoords = Vec::new();
    for model in models {
        let index_offset = positions.len() as u32; // Offset of the indices
        indices.reserve(model.mesh.indices.len());
        positions.reserve(model.mesh.positions.len() / 3);
        normals.reserve(model.mesh.normals.len() / 3);
        texcoords.reserve(model.mesh.texcoords.len() / 2);
        positions.extend(
            model
                .mesh
                .positions
                .chunks_exact(3)
                .map(|v| [v[0], v[1], v[2]]),
        );
        normals.extend(
            model
                .mesh
                .normals
                .chunks_exact(3)
                .map(|n| [n[0], n[1], n[2]]),
        );
        texcoords.extend(
            model
                .mesh
                .texcoords
                .chunks_exact(2)
                .map(|t| [t[0], 1.0 - t[1]]),
        );
        indices.extend(model.mesh.indices.iter().map(|i| i + index_offset));
    }

    let vertices = positions
        .iter()
        .zip(normals.iter())
        .zip(texcoords.iter())
        .map(|((&position, &normal), &texcoord)| Vertex {
            position: position.into(),
            normal: normal.into(),
            texcoord: texcoord.into(),
            color: normal.into(),
        })
        .collect();
    let mesh = Mesh::new(vertices, indices);
    Ok(Model::new(vec![mesh]))
}

async fn load_obj_data<'a, 'b>(
    mut bytes: &'a [u8],
    load_context: &'a mut LoadContext<'b>,
) -> tobj::LoadResult {
    let options = tobj::GPU_LOAD_OPTIONS;
    tobj::load_obj_buf_async(&mut bytes, &options, |p| async {
        // We don't use the MTL material as an asset, just load the bytes of it.
        // But we are unable to call ctx.finish() and feed the result back. (which is no new asset)
        // Is this allowed?
        let mut ctx = load_context.begin_labeled_asset();
        let path =
            PathBuf::from(ctx.asset_path().to_string()).with_file_name(p);
        let asset_path = AssetPath::from(path.to_string_lossy().into_owned());
        ctx.read_asset_bytes(&asset_path)
            .await
            .map_or(Err(tobj::LoadError::OpenFileFailed), |bytes| {
                tobj::load_mtl_buf(&mut bytes.as_slice())
            })
    })
    .await
}

/*
fn load_mat_texture(
    texture: &Option<String>,
    load_context: &mut LoadContext,
) -> Option<Handle<Image>> {
    if let Some(texture) = texture {
        let path = PathBuf::from(load_context.asset_path().to_string())
            .with_file_name(texture);
        let asset_path = AssetPath::from(path.to_string_lossy().into_owned());
        Some(load_context.load(&asset_path))
    } else {
        None
    }
}
*/

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
            .init_asset::<Model>()
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

    let mut meshes = Vec::new();
    for model in models {
        let mesh = &model.mesh;
        let mut vertices = Vec::new();

        for i in &mesh.indices {
            let pos = &mesh.positions;
            let nor = &mesh.normals;
            let tex = &mesh.texcoords;

            let i = *i as usize;
            let p = Vec3::new(pos[3 * i], pos[3 * i + 1], pos[3 * i + 2]);
            let n = if !nor.is_empty() {
                Vec3::new(nor[3 * i], nor[3 * i + 1], nor[3 * i + 2])
            } else {
                Vec3::ZERO
            };
            let t = if !tex.is_empty() {
                Vec2::new(tex[2 * i], 1.0 - tex[2 * i + 1])
            } else {
                Vec2::ZERO
            };

            vertices.push(Vertex {
                position: p,
                normal: n,
                color: n,
                texcoord: t,
            });
        }

        let indices = (0..vertices.len() as u32).collect();

        /*
        if let Some(material_id) = mesh.material_id {
            let material = &materials[material_id];
            bevy::log::info!("Material: {:#?}", material);
        }
        */

        /*
        // Process material
        if let Some(material_id) = mesh.material_id {
            let material = &materials[material_id];

            // Diffuse map
            if let Some(filename) = &material.diffuse_texture {
                //log::info!("Diffuse map: {}", filename);
            }

            // Specular map
            if let Some(filename) = &material.specular_texture {
                //log::info!("Specular map: {}", filename);
            }

            // Normal map
            if let Some(filename) = &material.normal_texture {
                //log::info!("Normal map: {}", filename);
            }

            // NOTE: no height maps for now
        }
        */

        let mesh = Mesh::new(vertices, indices);
        meshes.push(mesh);
    }

    Ok(Model::new(meshes))
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

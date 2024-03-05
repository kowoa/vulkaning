use bevy::asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext};
use bevy::prelude::*;
use bevy_utils::{BoxedFuture, HashMap};
use color_eyre::eyre::Result;
use std::{fs::File, io::BufReader};
use thiserror::Error;

use crate::renderer::mesh::Mesh;
use crate::renderer::model::Model;
use crate::renderer::vertex::Vertex;

const OBJ_EXTENSIONS: &[&str] = &["obj"];

pub struct ObjAssetsPlugin;
impl Plugin for ObjAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.preregister_asset_loader::<ObjLoader>(OBJ_EXTENSIONS);
        app.init_asset::<Model>();
        app.init_resource::<ObjAssetsLoading>();
    }

    fn finish(&self, app: &mut App) {
        app.register_asset_loader(ObjLoader);
    }
}

#[derive(Resource, Default)]
pub struct ObjAssetsLoading(pub HashMap<String, UntypedHandle>);

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
        &["obj"]
    }
}

#[allow(clippy::derivable_impls)] // TODO remove?
impl Default for ObjLoader {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Error, Debug)]
enum ObjError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid OBJ file: {0}")]
    InvalidFile(#[from] tobj::LoadError),
}

async fn load_obj<'a, 'b>(
    bytes: &'a [u8],
    _load_context: &'a mut LoadContext<'b>,
) -> Result<Model, ObjError> {
    load_obj_from_bytes(bytes)
}

fn load_obj_from_bytes(mut bytes: &[u8]) -> Result<Model, ObjError> {
    let options = tobj::GPU_LOAD_OPTIONS;
    let obj = tobj::load_obj_buf(&mut bytes, &options, |_| {
        Err(tobj::LoadError::GenericFailure)
    })?;

    let mut indices = Vec::new();
    let mut vertex_position = Vec::new();
    let mut vertex_normal = Vec::new();
    let mut vertex_texture = Vec::new();
    for model in obj.0 {
        let index_offset = vertex_position.len() as u32; // Offset of the indices
        indices.reserve(model.mesh.indices.len());
        vertex_position.reserve(model.mesh.positions.len() / 3);
        vertex_normal.reserve(model.mesh.normals.len() / 3);
        vertex_texture.reserve(model.mesh.texcoords.len() / 2);
        vertex_position.extend(
            model
                .mesh
                .positions
                .chunks_exact(3)
                .map(|v| [v[0], v[1], v[2]]),
        );
        vertex_normal.extend(
            model
                .mesh
                .normals
                .chunks_exact(3)
                .map(|n| [n[0], n[1], n[2]]),
        );
        vertex_texture.extend(
            model
                .mesh
                .texcoords
                .chunks_exact(2)
                .map(|t| [t[0], 1.0 - t[1]]),
        );
        indices.extend(model.mesh.indices.iter().map(|i| i + index_offset));
    }

    let vertices = {
        let mut vertices = Vec::with_capacity(vertex_position.len());
        for (i, pos) in vertex_position.iter().enumerate() {
            vertices.push(Vertex {
                position: (*pos).into(),
                normal: vertex_normal[i].into(),
                color: vertex_normal[i].into(),
                texcoord: vertex_texture[i].into(),
            });
        }
        vertices
    };

    let mesh = Mesh::new(vertices, indices);
    let model = Model::new(vec![mesh]);

    Ok(model)
}

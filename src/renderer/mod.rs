pub mod plugins;

mod vkinit;
mod vkutils;

mod buffer;
mod camera;
mod core;
mod descriptors;
mod frame;
mod image;
mod inner;
mod material;
mod mesh;
mod model;
mod queue_family_indices;
mod render_resources;
mod shader;
mod swapchain;
mod texture;
mod upload_context;
mod vertex;

mod gpu_data;

use bevy::ecs::system::Resource;
use color_eyre::eyre::{eyre, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use self::{
    camera::Camera, inner::RendererInner, model::Model,
    texture::TextureAssetData,
};

pub static mut ASSETS_DIR: Option<String> = None;
pub static mut SHADERBUILD_DIR: Option<String> = None;

#[derive(Default, Resource)]
pub struct AssetData {
    models: HashMap<String, Model>,
    textures: HashMap<String, TextureAssetData>,
}

#[derive(Clone, Resource)]
pub struct Renderer {
    inner: Option<Arc<Mutex<RendererInner>>>,
}

impl Renderer {
    pub fn new(window: &winit::window::Window) -> Result<Self> {
        Ok(Self {
            inner: Some(Arc::new(Mutex::new(RendererInner::new(window)?))),
        })
    }

    pub fn init_resources(&self, assets: &mut AssetData) -> Result<()> {
        if let Some(inner) = &self.inner {
            inner.lock().unwrap().init_resources(assets)
        } else {
            Err(eyre!("Failed to init resources because renderer has already been destroyed"))
        }
    }

    pub fn draw_frame(&self, camera: &Camera) -> Result<()> {
        if let Some(inner) = &self.inner {
            inner.lock().unwrap().draw_frame(camera)
        } else {
            Err(eyre!("Failed to draw frame because renderer has already been destroyed"))
        }
    }

    pub fn cleanup(&mut self) {
        if let Some(inner) = self.inner.take() {
            let inner = match Arc::try_unwrap(inner) {
                Ok(inner) => Ok(inner),
                Err(_) => Err(eyre!(
                    "Failed to cleanup because renderer is currently in use"
                )),
            }
            .unwrap();
            inner.into_inner().unwrap().cleanup();
        }
    }
}

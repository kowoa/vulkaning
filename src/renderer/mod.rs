pub mod plugins;
pub mod render_resources;

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
mod render_object;
mod resources;
mod shader;
mod swapchain;
mod texture;
mod upload_context;
mod vertex;

use bevy::ecs::system::Resource;
use color_eyre::eyre::{eyre, Result};
use gpu_allocator::vulkan::Allocator;
use std::{
    ffi::CString,
    sync::{Arc, Mutex},
};

use self::{
    camera::Camera, inner::RendererInner, render_resources::RenderResources,
};

pub static mut ASSETS_DIR: Option<String> = None;
pub static mut SHADERBUILD_DIR: Option<String> = None;

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

    pub fn upload_resources(
        &self,
        resources: &mut RenderResources,
    ) -> Result<()> {
        if let Some(inner) = &self.inner {
            inner.lock().unwrap().upload_resources(resources)
        } else {
            Err(eyre!("Failed to init resources because renderer has already been destroyed"))
        }
    }

    pub fn draw_frame(
        &self,
        camera: &Camera,
        resources: &RenderResources,
    ) -> Result<()> {
        if let Some(inner) = &self.inner {
            inner.lock().unwrap().draw_frame(camera, resources)
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
            let inner = inner.into_inner().unwrap();
            inner.cleanup();
        }
    }

    pub fn get_background_index(&self) -> u32 {
        self.inner
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .resources
            .as_ref()
            .unwrap()
            .current_background_effects_index as u32
    }

    pub fn set_background_index(&mut self, new_index: u32) {
        self.inner
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .resources
            .as_mut()
            .unwrap()
            .current_background_effects_index = new_index as usize;
    }
}

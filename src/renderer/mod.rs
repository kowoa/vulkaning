pub mod plugin;

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

use color_eyre::eyre::{eyre, Result};
use egui_ash::{AshRenderState, EguiCommand};
use gpu_allocator::vulkan::Allocator;
use std::{
    ffi::CString,
    sync::{Arc, Mutex},
};

use self::inner::RendererInner;

pub static mut ASSETS_DIR: Option<String> = None;
pub static mut SHADERBUILD_DIR: Option<String> = None;

#[derive(Clone)]
pub struct Renderer {
    inner: Option<Arc<Mutex<RendererInner>>>,
}

impl Renderer {
    pub fn new(window: &winit::window::Window) -> Result<Self> {
        Ok(Self {
            inner: Some(Arc::new(Mutex::new(RendererInner::new(window)?))),
        })
    }

    pub fn draw_frame(
        &self,
        width: u32,
        height: u32,
        egui_cmd: Option<EguiCommand>,
    ) -> Result<u32> {
        if let Some(inner) = &self.inner {
            inner.lock().unwrap().draw_frame(width, height, egui_cmd)
        } else {
            Err(eyre!("Failed to draw frame because renderer has already been destroyed"))
        }
    }

    pub fn present_frame(&self, swapchain_image_index: u32) -> Result<()> {
        if let Some(inner) = &self.inner {
            inner.lock().unwrap().present_frame(swapchain_image_index)
        } else {
            Err(eyre!("Failed to present frame because renderer has already been destroyed"))
        }
    }

    pub fn ash_render_state(&self) -> AshRenderState<Arc<Mutex<Allocator>>> {
        let inner = self.inner.as_ref().unwrap().lock().unwrap();
        AshRenderState {
            entry: inner.core.entry.clone(),
            instance: inner.core.instance.clone(),
            physical_device: inner.core.physical_device,
            device: inner.core.device.clone(),
            surface_loader: inner.core.surface_loader.clone(),
            swapchain_loader: inner.swapchain.swapchain_loader.clone(),
            queue: inner.core.graphics_queue,
            queue_family_index: inner
                .core
                .queue_family_indices
                .get_graphics_family()
                .unwrap(),
            command_pool: inner.command_pool,
            allocator: inner.core.get_allocator_ref(),
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
            .current_background_effects_index as u32
    }

    pub fn set_background_index(&mut self, new_index: u32) {
        self.inner
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .resources
            .current_background_effects_index = new_index as usize;
    }
}

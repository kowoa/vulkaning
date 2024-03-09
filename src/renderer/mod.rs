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
mod shader;
mod swapchain;
mod texture;
mod upload_context;
mod vertex;

mod gpu_data;

use ash::vk;
use bevy::ecs::system::Resource;
use color_eyre::eyre::{eyre, Result};
use std::sync::{Arc, Mutex};

use self::{
    camera::Camera, inner::RendererInner, render_resources::RenderResources,
};

pub static mut ASSETS_DIR: Option<String> = None;
pub static mut SHADERBUILD_DIR: Option<String> = None;

struct DrawContext<'a> {
    cmd: vk::CommandBuffer,
    device: &'a ash::Device,
    allocator: &'a mut gpu_allocator::vulkan::Allocator,
    camera: &'a Camera,
    frame_number: u32,
    swapchain: &'a swapchain::Swapchain,
    desc_set_layouts: &'a descriptors::DescriptorSetLayouts,
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

    pub fn init_resources(
        &self,
        resources: &mut RenderResources,
    ) -> Result<()> {
        if let Some(inner) = &self.inner {
            inner.lock().unwrap().init_resources(resources)
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

    pub fn cleanup(&mut self, resources: &mut RenderResources) {
        if let Some(inner) = self.inner.take() {
            let inner = match Arc::try_unwrap(inner) {
                Ok(inner) => Ok(inner),
                Err(_) => Err(eyre!(
                    "Failed to cleanup because renderer is currently in use"
                )),
            }
            .unwrap();
            let inner = inner.into_inner().unwrap();
            inner.cleanup(resources);
        }
    }
}

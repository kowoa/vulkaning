use std::sync::Arc;

use ash::vk;
use glam::Mat4;

use super::{inner::DrawContext, material::MaterialInstance};

/// A completely flattened abstraction of the params needed for a single vkCmdDrawIndexed call
pub struct RenderObject {
    index_count: u32,
    first_index: u32,
    index_buffer: vk::Buffer,
    material: Arc<MaterialInstance>,
    transform: Mat4,
    vertex_buffer_address: vk::DeviceAddress,
}

trait Renderable {
    fn draw(&self, parent: Mat4, ctx: &DrawContext);
}

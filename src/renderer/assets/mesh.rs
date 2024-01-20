use crate::renderer::assets::vertex::Vertex;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec4};
use gpu_allocator::vulkan::Allocator;

use crate::renderer::memory::AllocatedBuffer;

#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct MeshPushConstants {
    pub data: Vec4,
    pub render_matrix: Mat4,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub vertex_buffer: AllocatedBuffer,
}

impl Mesh {
    pub fn new(
        vertices: Vec<Vertex>,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> anyhow::Result<Self> {
        let vertex_buffer =
            AllocatedBuffer::new(&vertices, device, allocator)?;
        Ok(Self {
            vertices,
            vertex_buffer,
        })
    }

    pub fn cleanup(
        self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) {
        log::info!("Cleaning up mesh ...");
        self.vertex_buffer.cleanup(device, allocator);
    }
}

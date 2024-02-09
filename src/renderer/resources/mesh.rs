use std::sync::atomic::{AtomicUsize, Ordering};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec4};
use gpu_allocator::vulkan::Allocator;
use color_eyre::eyre::Result;

use crate::renderer::memory::AllocatedBuffer;

use super::vertex::Vertex;

#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct MeshPushConstants {
    pub data: Vec4,
    pub render_matrix: Mat4,
}

static MESH_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct Mesh {
    pub id: usize,
    pub vertices: Vec<Vertex>,
    pub vertex_buffer: AllocatedBuffer,
}

impl PartialEq for Mesh {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Mesh {
    pub fn new(
        vertices: Vec<Vertex>,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let id = MESH_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let vertex_buffer =
            AllocatedBuffer::new_vertex_buffer(&vertices, device, allocator)?;
        Ok(Self {
            id,
            vertices,
            vertex_buffer,
        })
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        log::info!("Cleaning up mesh ...");
        self.vertex_buffer.cleanup(device, allocator);
    }

    pub fn new_triangle(
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let vertices = vec![
            Vertex {
                position: [-0.5, -0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [1.0, 0.0, 0.0].into(),
            },
            Vertex {
                position: [0.5, -0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [0.0, 1.0, 0.0].into(),
            },
            Vertex {
                position: [0.0, 0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [0.0, 0.0, 1.0].into(),
            },
        ];

        Self::new(vertices, device, allocator)
    }
}

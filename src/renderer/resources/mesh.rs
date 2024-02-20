use std::sync::{
    atomic::{AtomicUsize, Ordering},
    MutexGuard,
};

use ash::vk;
use bytemuck::{Pod, Zeroable};
use color_eyre::eyre::{eyre, Result};
use glam::{Mat4, Vec4};
use gpu_allocator::vulkan::Allocator;

use crate::renderer::{memory::AllocatedBuffer, UploadContext};

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
    pub vertex_buffer: Option<AllocatedBuffer>,
}

impl PartialEq for Mesh {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>) -> Self {
        let id = MESH_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            vertices,
            vertex_buffer: None,
        }
    }

    pub fn upload(
        &mut self,
        device: &ash::Device,
        allocator: &mut MutexGuard<Allocator>,
        upload_context: &UploadContext,
    ) -> Result<()> {
        let buffer_size =
            (self.vertices.len() * std::mem::size_of::<Vertex>()) as u64;
        // Create CPU-side staging buffer
        let mut staging_buffer = AllocatedBuffer::new(
            device,
            allocator,
            buffer_size as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
            "Mesh staging buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        // Copy vertex data into staging buffer
        let _ = staging_buffer.write(&self.vertices[..], 0)?;

        // Create GPU-side vertex buffer if it doesn't already exist
        if self.vertex_buffer.is_none() {
            self.vertex_buffer = Some(AllocatedBuffer::new(
                device,
                allocator,
                buffer_size,
                // Use this buffer to render meshes and copy data into
                vk::BufferUsageFlags::VERTEX_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
                "Mesh vertex buffer",
                gpu_allocator::MemoryLocation::GpuOnly,
            )?);
        }

        // Execute immediate command to transfer data from staging buffer to vertex buffer
        if let Some(vertex_buffer) = &self.vertex_buffer {
            upload_context.immediate_submit(
                |cmd: &vk::CommandBuffer, device: &ash::Device| {
                    let copy = vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: buffer_size,
                    };
                    unsafe {
                        device.cmd_copy_buffer(
                            *cmd,
                            staging_buffer.buffer,
                            vertex_buffer.buffer,
                            &[copy],
                        );
                    }
                },
                device,
            )?;

            // At this point, the vertex buffer should be populated with data from the staging buffer
            // Destroy staging buffer now because the vertex buffer now holds the data
            staging_buffer.cleanup(device, allocator);

            Ok(())
        } else {
            Err(eyre!("Vertex buffer not created"))
        }
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        log::info!("Cleaning up mesh ...");
        if let Some(vertex_buffer) = self.vertex_buffer {
            vertex_buffer.cleanup(device, allocator);
        }
    }

    pub fn new_triangle() -> Self {
        let vertices = vec![
            Vertex {
                position: [-0.5, -0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [1.0, 0.0, 0.0].into(),
                texcoord: [0.0, 0.0].into(),
            },
            Vertex {
                position: [0.5, -0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [0.0, 1.0, 0.0].into(),
                texcoord: [0.5, 1.0].into(),
            },
            Vertex {
                position: [0.0, 0.5, 0.0].into(),
                normal: [0.0, 0.0, 1.0].into(),
                color: [0.0, 0.0, 1.0].into(),
                texcoord: [1.0, 0.0].into(),
            },
        ];

        Self::new(vertices)
    }
}

use ash::vk::DeviceMemory;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use gpu_alloc::{GpuAllocator, MemoryBlock, Request, UsageFlags};
use gpu_alloc_ash::AshMemoryDevice;

use crate::renderer::core::Core;

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    mem_block: MemoryBlock<DeviceMemory>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, core: &mut Core) -> anyhow::Result<Self> {
        let mut mem_block = unsafe {
            core.allocator.alloc(
                AshMemoryDevice::wrap(&core.device),
                Request {
                    size: vertices.len() as u64
                        * std::mem::size_of::<Vertex>() as u64,
                    align_mask: 0,
                    usage: UsageFlags::UPLOAD,
                    memory_types: !0,
                },
            )?
        };

        unsafe {
            mem_block.write_bytes(
                AshMemoryDevice::wrap(&core.device),
                0,
                bytemuck::cast_slice(&vertices),
            )?;
        }

        Ok(Self {
            vertices,
            mem_block,
        })
    }

    pub fn cleanup(
        mut self,
        device: &ash::Device,
        allocator: &mut GpuAllocator<DeviceMemory>,
    ) {
        log::info!("Cleaning up mesh ...");
        unsafe {
            allocator.dealloc(AshMemoryDevice::wrap(device), self.mem_block);
        }
    }
}

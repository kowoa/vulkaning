use ash::vk::DeviceMemory;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use gpu_alloc::MemoryBlock;

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub mem_block: MemoryBlock<DeviceMemory>,
}

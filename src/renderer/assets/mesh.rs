use ash::vk::DeviceMemory;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use gpu_alloc::{MemoryBlock, GpuAllocator};
use gpu_alloc_ash::AshMemoryDevice;

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

impl Mesh {
    pub fn destroy(self, device: &ash::Device, allocator: &mut GpuAllocator<DeviceMemory>) {
        log::info!("Cleaning up mesh ...");
        unsafe {
            allocator.dealloc(AshMemoryDevice::wrap(device), self.mem_block);
        }
    }
}
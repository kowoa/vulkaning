use ash::vk;
use glam::Vec3A;

use super::buffer::AllocatedBuffer;

pub struct Vertex {
    pub position: Vec3A,
    pub normal: Vec3A,
    pub color: Vec3A,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub vertex_buffer: AllocatedBuffer,
}

impl Mesh {
    pub fn destroy(self, device: &ash::Device, allocator: &mut gpu_allocator::vulkan::Allocator) {
        self.vertex_buffer.destroy(device, allocator);
    }
}
use ash::vk;
use glam::Vec3A;

use crate::renderer::vk_types::AllocatedBuffer;

pub struct Vertex {
    position: Vec3A,
    normal: Vec3A,
    color: Vec3A,
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    vertex_buffer: AllocatedBuffer,
}
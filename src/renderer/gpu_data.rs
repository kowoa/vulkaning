// This file contains data structures sent to the GPU

use ash::vk;
use glam::{Mat4, Vec3, Vec4};

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct GpuVertexData {
    pub position: Vec3,
    pub uv_x: f32,
    pub normal: Vec3,
    pub uv_y: f32,
    pub color: Vec4,
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct GpuSceneData {
    pub cam_data: GpuCameraData,
    pub ambient_color: Vec4,
    pub sunlight_direction: Vec4,
    pub sunlight_color: Vec4,
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct GpuCameraData {
    pub viewproj: Mat4,
    pub near: f32,
    pub far: f32,
}

#[repr(C)]
/// Push constants for mesh object draws
pub struct GpuDrawPushConstants {
    world_matrix: Mat4,
    vertex_buffer: vk::DeviceAddress,
}

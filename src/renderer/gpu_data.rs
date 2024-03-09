// This file contains data structures sent to the GPU

use glam::{Mat4, Vec4};

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

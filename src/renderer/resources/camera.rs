use glam::Mat4;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GpuCameraData {
    pub view: Mat4,
    pub proj: Mat4,
    pub viewproj: Mat4,
}

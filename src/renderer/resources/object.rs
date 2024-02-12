use glam::Mat4;

#[repr(C)]
pub struct GpuObjectData {
    pub model_mat: Mat4
}

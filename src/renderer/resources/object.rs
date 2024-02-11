use glam::Mat4;

#[repr(C)]
pub struct GpuObjectData {
    model_mat: Mat4
}

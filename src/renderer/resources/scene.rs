use glam::Vec4;

#[derive(Default, Copy, Clone)]
pub struct GpuSceneData {
    pub fog_color: Vec4, // w for exponent
    pub fog_distances: Vec4, // x for min, y for max, zw unused
    pub ambient_color: Vec4,
    pub sunlight_direction: Vec4, // w for sun power
    pub sunlight_color: Vec4,
}

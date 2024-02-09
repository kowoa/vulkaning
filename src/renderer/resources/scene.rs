use glam::Vec4;

pub struct GpuSceneData {
    fog_color: Vec4, // w for exponent
    fog_distances: Vec4, // x for min, y for max, zw unused
    ambient_color: Vec4,
    sunlight_direction: Vec4, // w for sun power
    sunlight_color: Vec4,
}

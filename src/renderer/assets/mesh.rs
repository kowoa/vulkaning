use glam::Vec3;

#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
}

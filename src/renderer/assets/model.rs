use ash::vk::DeviceMemory;
use glam::{Vec3, Vec2};
use gpu_alloc::GpuAllocator;

use crate::renderer::{core::Core, assets::mesh::Vertex};

use super::mesh::Mesh;

pub struct Model {
    pub meshes: Vec<Mesh>,
}

impl Model {
    pub fn load_from_obj(filename: &str, core: &mut Core) -> anyhow::Result<Self> {
        println!("HAHAHA");
        let (models, materials) =
            tobj::load_obj(filename, &tobj::LoadOptions {
                single_index: true,
                ..Default::default()
            })?;
        println!("BEFORE");
        let materials = materials?;
        println!("AFTER");

        log::info!("Number of models: {}", models.len());
        log::info!("Number of materials: {}", materials.len());

        let mut meshes = Vec::new();
        for model in models {
            let mesh = &model.mesh;
            let mut vertices = Vec::new();

            for i in &mesh.indices {
                let pos = &mesh.positions;
                let nor = &mesh.normals;
                let tex = &mesh.texcoords;

                let i = *i as usize;
                let p = Vec3::new(pos[3*i], pos[3*i+1], pos[3*i+2]);
                let n = if !nor.is_empty() {
                    Vec3::new(nor[3*i], nor[3*i+1], nor[3*i+2])
                } else {
                    Vec3::ZERO
                };
                let t = if !tex.is_empty() {
                    Vec2::new(tex[2*i], 1.0-tex[2*i+1])
                } else {
                    Vec2::ZERO
                };

                vertices.push(Vertex {
                    position: p,
                    normal: n,
                    color: Vec3::new(1.0, 0.0, 0.0),
                });
            }

            let mesh = Mesh::new(vertices, core)?;
            meshes.push(mesh);
        }

        Ok(Self { meshes })
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut GpuAllocator<DeviceMemory>) {
        for mesh in self.meshes {
            mesh.cleanup(device, allocator);
        }
    }

}

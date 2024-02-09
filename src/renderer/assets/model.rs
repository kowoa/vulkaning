use glam::{Vec3, Vec2};
use gpu_allocator::vulkan::Allocator;
use color_eyre::eyre::Result;

use crate::renderer::{core::Core, assets::vertex::Vertex};

use super::mesh::Mesh;

#[derive(Default, PartialEq)]
pub struct Model {
    pub meshes: Vec<Mesh>,
}

impl Model {
    pub fn new(meshes: Vec<Mesh>) -> Self {
        Self { meshes }
    }

    pub fn load_from_obj(
        filename: &str,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let (models, materials) =
            tobj::load_obj(filename, &tobj::LoadOptions {
                single_index: true,
                ..Default::default()
            })?;
        let materials = materials?;

        log::info!("Number of models: {}", models.len());
        log::info!("Number of materials: {}", materials.len());

        let mut meshes = Vec::new();
        for model in models {
            let mesh = &model.mesh;
            let mut vertices = Vec::new();

            const COLORS: [Vec3; 3] = [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ];

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
                    color: COLORS[i%3],
                });
            }

            // Process material
            if let Some(material_id) = mesh.material_id {
                let material = &materials[material_id];

                // Diffuse map
                if let Some(filename) = &material.diffuse_texture {
                    log::info!("Diffuse map: {}", filename);
                }

                // Specular map
                if let Some(filename) = &material.specular_texture {
                    log::info!("Specular map: {}", filename);
                }

                // Normal map
                if let Some(filename) = &material.normal_texture {
                    log::info!("Normal map: {}", filename);
                }

                // NOTE: no height maps for now
            }

            let mesh = Mesh::new(vertices, device, allocator)?;
            meshes.push(mesh);
        }

        Ok(Self { meshes })
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        for mesh in self.meshes {
            mesh.cleanup(device, allocator);
        }
    }

}

use bevy::{asset::Asset, log, reflect::TypePath};
use std::{path::PathBuf, sync::MutexGuard};

use ash::vk;
use color_eyre::eyre::{eyre, OptionExt, Result};
use glam::{Vec2, Vec3};
use gpu_allocator::vulkan::Allocator;

use crate::renderer::{
    buffer::AllocatedBuffer, upload_context::UploadContext, vertex::Vertex,
    ASSETS_DIR,
};

use super::mesh::Mesh;

#[derive(Asset, TypePath, Debug)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub vertex_buffer: Option<AllocatedBuffer>,
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.meshes
            .iter()
            .zip(other.meshes.iter())
            .all(|(mesh, other)| mesh == other)
    }
}

impl Model {
    pub fn new(meshes: Vec<Mesh>) -> Self {
        Self {
            meshes,
            vertex_buffer: None,
        }
    }

    pub fn draw(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
    ) -> Result<()> {
        self.bind_vertex_buffer(cmd, device)?;

        // Draw this render object's model
        let vertex_count: u32 = self
            .meshes
            .iter()
            .map(|mesh| mesh.vertices.len() as u32)
            .sum();
        let index_count = self.meshes[0].indices.len() as u32;
        unsafe {
            device.cmd_draw(cmd, vertex_count, 1, 0, 0);
            //device.cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
        }

        Ok(())
    }

    fn bind_vertex_buffer(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
    ) -> Result<()> {
        let buffer = self
            .vertex_buffer
            .as_ref()
            .ok_or_eyre("No vertex buffer found")?;

        // Bind vertex buffer
        unsafe {
            device.cmd_bind_vertex_buffers(cmd, 0, &[buffer.buffer], &[0]);
        }

        Ok(())
    }

    pub fn load_from_obj(filename: &str) -> Result<Self> {
        let filepath = unsafe {
            let mut path = PathBuf::from(
                ASSETS_DIR
                    .as_ref()
                    .ok_or_eyre("Assets directory not specified")?
                    .clone(),
            );
            path.push(filename);
            path
        };

        let (models, materials) = tobj::load_obj(
            filepath,
            &tobj::LoadOptions {
                single_index: true,
                triangulate: true,
                ..Default::default()
            },
        )?;
        let materials = materials?;

        log::info!("Number of models: {}", models.len());
        log::info!("Number of materials: {}", materials.len());

        let mut meshes = Vec::new();
        for model in models {
            let mesh = &model.mesh;
            let mut vertices = Vec::new();
            let mut indices = Vec::new();

            for i in &mesh.indices {
                let pos = &mesh.positions;
                let nor = &mesh.normals;
                let tex = &mesh.texcoords;

                let i = *i as usize;
                let p = Vec3::new(pos[3 * i], pos[3 * i + 1], pos[3 * i + 2]);
                let n = if !nor.is_empty() {
                    Vec3::new(nor[3 * i], nor[3 * i + 1], nor[3 * i + 2])
                } else {
                    Vec3::ZERO
                };
                let t = if !tex.is_empty() {
                    Vec2::new(tex[2 * i], 1.0 - tex[2 * i + 1])
                } else {
                    Vec2::ZERO
                };

                vertices.push(Vertex {
                    position: p,
                    normal: n,
                    color: n,
                    texcoord: t,
                });
                indices.push(i as u32);
            }

            // Process material
            if let Some(material_id) = mesh.material_id {
                let material = &materials[material_id];

                // Diffuse map
                if let Some(filename) = &material.diffuse_texture {
                    //log::info!("Diffuse map: {}", filename);
                }

                // Specular map
                if let Some(filename) = &material.specular_texture {
                    //log::info!("Specular map: {}", filename);
                }

                // Normal map
                if let Some(filename) = &material.normal_texture {
                    //log::info!("Normal map: {}", filename);
                }

                // NOTE: no height maps for now
            }

            let mesh = Mesh::new(vertices, indices);
            meshes.push(mesh);
        }

        Ok(Self::new(meshes))
    }

    pub fn upload(
        &mut self,
        device: &ash::Device,
        allocator: &mut Allocator,
        upload_context: &UploadContext,
    ) -> Result<()> {
        let vertices = self
            .meshes
            .iter()
            .flat_map(|mesh| mesh.vertices.iter().cloned())
            .collect::<Vec<Vertex>>();

        let buffer_size =
            (vertices.len() * std::mem::size_of::<Vertex>()) as u64;
        // Create CPU-side staging buffer
        let mut staging_buffer = AllocatedBuffer::new(
            device,
            allocator,
            buffer_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            "Model staging buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        // Copy vertex data into staging buffer
        let _ = staging_buffer.write(&vertices[..], 0)?;

        // Create GPU-side vertex buffer if it doesn't already exist
        if self.vertex_buffer.is_none() {
            self.vertex_buffer = Some(AllocatedBuffer::new(
                device,
                allocator,
                buffer_size,
                // Use this buffer to render meshes and copy data into
                vk::BufferUsageFlags::VERTEX_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
                "Model vertex buffer",
                gpu_allocator::MemoryLocation::GpuOnly,
            )?);
        }

        // Execute immediate command to transfer data from staging buffer to vertex buffer
        if let Some(vertex_buffer) = &self.vertex_buffer {
            upload_context.immediate_submit(
                |cmd: &vk::CommandBuffer, device: &ash::Device| {
                    let copy = vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: buffer_size,
                    };
                    unsafe {
                        device.cmd_copy_buffer(
                            *cmd,
                            staging_buffer.buffer,
                            vertex_buffer.buffer,
                            &[copy],
                        );
                    }
                },
                device,
            )?;

            // At this point, the vertex buffer should be populated with data from the staging buffer
            // Destroy staging buffer now because the vertex buffer now holds the data
            staging_buffer.cleanup(device, allocator);

            Ok(())
        } else {
            staging_buffer.cleanup(device, allocator);
            Err(eyre!("Vertex buffer not created"))
        }
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        if let Some(vertex_buffer) = self.vertex_buffer {
            vertex_buffer.cleanup(device, allocator);
        }
    }
}

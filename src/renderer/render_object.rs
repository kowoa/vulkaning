use ash::vk;
use bevy::log;
use color_eyre::eyre::{OptionExt, Result};
use std::sync::Arc;

use glam::{Mat4, Vec4};

use crate::renderer::{
    buffer::AllocatedBuffer, material::Material, mesh::MeshPushConstants,
    model::Model,
};

use super::{frame::Frame, texture::Texture};

pub struct RenderObject<'a> {
    pub model: &'a Model,
    pub material: Arc<Material>,
    pub texture: Option<Arc<Texture>>,
    pub transform: Mat4,
}

impl<'a> RenderObject<'a> {
    pub fn new(
        model: &Model,
        material: Arc<Material>,
        texture: Option<Arc<Texture>>,
        transform: Mat4,
    ) -> Self {
        Self {
            model,
            material,
            texture,
            transform,
        }
    }

    pub fn draw(
        &self,
        device: &ash::Device,
        frame: &Frame,
        frame_index: u32,
        last_model_drawn: &mut Option<Arc<Model>>,
        last_material_drawn: &mut Option<Arc<Material>>,
        scene_camera_buffer: &AllocatedBuffer,
        instance_index: u32,
    ) -> Result<()> {
        let cmd = frame.command_buffer;

        // Update pipeline
        {
            let should_update_pipeline = if let Some(last) = last_material_drawn
            {
                self.material.pipeline != last.pipeline
            } else {
                true
            };

            // Only bind the pipeline if it doesn't match the already bound one
            if should_update_pipeline {
                unsafe {
                    device.cmd_bind_pipeline(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.material.pipeline,
                    );
                }
            }
        }

        // Update push constants
        {
            let constants = MeshPushConstants {
                data: Vec4::new(0.0, 0.0, 0.0, 0.0),
                render_matrix: self.transform,
            };

            unsafe {
                device.cmd_push_constants(
                    cmd,
                    self.material.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    bytemuck::bytes_of(&constants),
                );
            }
        }

        let should_update_model = if let Some(last) = last_model_drawn {
            self.model.as_ref() != last.as_ref()
        } else {
            true
        };

        let should_update_material = if let Some(last) = last_material_drawn {
            self.material.as_ref() != last.as_ref()
        } else {
            true
        };

        if should_update_model {
            // Update descriptor sets to use this model's pipeline
            if should_update_material {
                unsafe {
                    let scene_start_offset =
                        scene_camera_buffer.offsets.as_ref().unwrap()
                            [frame_index as usize];
                    let camera_start_offset =
                        scene_camera_buffer.offsets.as_ref().unwrap()
                            [frame_index as usize + 2];
                    // Bind global descriptor set
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.material.pipeline_layout,
                        0,
                        &[frame.global_desc_set],
                        &[scene_start_offset, camera_start_offset],
                    );
                }

                unsafe {
                    // Bind object descriptor set
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.material.pipeline_layout,
                        1,
                        &[frame.object_desc_set],
                        &[],
                    );
                }

                if let Some(texture) = &self.texture {
                    unsafe {
                        device.cmd_bind_descriptor_sets(
                            cmd,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.material.pipeline_layout,
                            2,
                            &[texture.desc_set()],
                            &[],
                        );
                    }
                }

                let _ = last_material_drawn.insert(self.material.clone());
            }

            // Update vertex buffer
            {
                let buffer = self
                    .model
                    .vertex_buffer
                    .as_ref()
                    .ok_or_eyre("No vertex buffer found")?;

                // Bind vertex buffer
                unsafe {
                    device.cmd_bind_vertex_buffers(
                        cmd,
                        0,
                        &[buffer.buffer],
                        &[0],
                    );
                }
            }

            let _ = last_model_drawn.insert(Arc::clone(&self.model));
        }

        // Draw this render object's model
        let vertex_count = self
            .model
            .meshes
            .iter()
            .map(|mesh| mesh.vertices.len() as u32)
            .sum();
        unsafe {
            device.cmd_draw(cmd, vertex_count, 1, 0, instance_index);
        }

        Ok(())
    }
}

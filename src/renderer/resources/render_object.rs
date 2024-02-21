use ash::vk;
use color_eyre::eyre::{eyre, OptionExt, Result};
use std::sync::Arc;

use glam::{Mat4, Vec4};

use crate::renderer::memory::AllocatedBuffer;

use super::{
    frame::Frame, mesh::MeshPushConstants, model::Model, pipeline::Pipeline,
};

pub struct RenderObject {
    pub model: Arc<Model>,
    pub pipeline: Arc<Pipeline>,
    pub transform: Mat4,
}

impl RenderObject {
    pub fn new(
        model: Arc<Model>,
        pipeline: Arc<Pipeline>,
        transform: Mat4,
    ) -> Self {
        Self {
            model,
            pipeline,
            transform,
        }
    }

    pub fn draw(
        &self,
        device: &ash::Device,
        frame: &Frame,
        frame_index: u32,
        last_model_drawn: &mut Option<Arc<Model>>,
        scene_camera_buffer: &AllocatedBuffer,
        instance_index: u32,
    ) -> Result<()> {
        let cmd = frame.command_buffer;

        // Update push constants
        {
            let constants = MeshPushConstants {
                data: Vec4::new(0.0, 0.0, 0.0, 0.0),
                render_matrix: self.transform,
            };

            unsafe {
                device.cmd_push_constants(
                    cmd,
                    self.pipeline.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    bytemuck::bytes_of(&constants),
                );
            }
        }

        // Only bind the mesh if it's a different one from last bind
        let should_update_model = if let Some(last) = last_model_drawn {
            self.model.as_ref() != last.as_ref()
        } else {
            true
        };

        if should_update_model {
            // Update descriptor sets to use this model's pipeline
            {
                let scene_start_offset =
                    scene_camera_buffer.offsets.as_ref().unwrap()
                        [frame_index as usize];
                let camera_start_offset =
                    scene_camera_buffer.offsets.as_ref().unwrap()
                        [frame_index as usize + 2];
                unsafe {
                    // Bind global descriptor set
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline.pipeline_layout,
                        0,
                        &[frame.global_desc_set],
                        &[scene_start_offset, camera_start_offset],
                    );

                    // Bind object descriptor set
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline.pipeline_layout,
                        1,
                        &[frame.object_desc_set],
                        &[],
                    );
                }
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

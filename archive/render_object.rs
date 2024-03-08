use std::sync::Arc;

use ash::vk;
use color_eyre::eyre::{OptionExt, Result};
use glam::{Mat4, Vec4};

use crate::renderer::{
    buffer::AllocatedBuffer, material::Material, mesh::MeshPushConstants,
    model::Model,
};

use super::{frame::Frame, texture::Texture};

pub struct RenderObject {
    pub model: Arc<Model>,
    pub material: Arc<Material>,
    pub texture: Option<Arc<Texture>>,
    pub transform: Mat4,
}

impl RenderObject {
    pub fn new(
        model: Arc<Model>,
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

/*
fn draw_render_objects(
    &mut self,
    width: u32,
    height: u32,
    first_index: usize,
    count: usize,
    camera: &Camera,
) -> Result<()> {
    let core = &self.core;
    let frame_index = self.frame_number % FRAME_OVERLAP;
    let scene_start_offset = core
        .pad_uniform_buffer_size(std::mem::size_of::<GpuSceneData>() as u64)
        * frame_index as u64;
    let camera_start_offset = core
        .pad_uniform_buffer_size(std::mem::size_of::<GpuSceneData>() as u64)
        * FRAME_OVERLAP as u64
        + core.pad_uniform_buffer_size(
            std::mem::size_of::<GpuCameraData>() as u64,
        ) * frame_index as u64;

    // Write into scene section of scene-camera uniform buffer
    {
        // Fill a GpuSceneData struct
        let framed = self.frame_number as f32 / 120.0;
        let scene_data = GpuSceneData {
            ambient_color: Vec4::new(framed.sin(), 0.0, framed.cos(), 1.0),
            ..Default::default()
        };

        // Copy GpuSceneData struct to buffer
        self.scene_camera_buffer
            .write(&[scene_data], scene_start_offset as usize)?;
    }

    // Write into camera section of scene-camera uniform buffer
    {
        // Fill a GpuCameraData struct
        let cam_data = GpuCameraData {
            viewproj: camera.viewproj_mat(width as f32, height as f32),
            near: camera.near,
            far: camera.far,
        };

        // Copy GpuCameraData struct to buffer
        self.scene_camera_buffer
            .write(&[cam_data], camera_start_offset as usize)?;
    }

    // Write into object storage buffer
    {
        //let rot = Mat4::from_rotation_y(self.frame_number as f32 / 240.0);
        let rot = Mat4::IDENTITY;
        let object_data = self
            .resources
            .as_ref()
            .unwrap()
            .render_objs
            .iter()
            .map(|obj| rot * obj.transform)
            .collect::<Vec<_>>();
        let mut frame = self.get_current_frame()?;
        frame.object_buffer.write(&object_data, 0)?;
    }

    let mut last_model_drawn = None;
    let mut last_material_drawn = None;
    for instance_index in first_index..(first_index + count) {
        let device = &core.device;
        let render_obj =
            &self.resources.as_ref().unwrap().render_objs[instance_index];
        let frame = self.get_current_frame()?;

        render_obj.draw(
            device,
            &frame,
            frame_index,
            &mut last_model_drawn,
            &mut last_material_drawn,
            &self.scene_camera_buffer,
            instance_index as u32,
        )?;
    }

    Ok(())
}
*/

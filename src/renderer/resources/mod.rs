// Asset initialization
pub mod camera;
pub mod frame;
pub mod mesh;
pub mod model;
pub mod object;
pub mod pipeline;
pub mod render_object;
pub mod renderpass;
pub mod scene;
pub mod shader;
pub mod vertex;

use color_eyre::eyre::Result;
use std::{collections::HashMap, mem::ManuallyDrop, rc::Rc};

use ash::vk;
use glam::{Mat4, Vec3, Vec4};
use gpu_allocator::vulkan::Allocator;
use mesh::Mesh;
use pipeline::PipelineBuilder;
use renderpass::Renderpass;
use shader::Shader;

use self::{
    camera::GpuCameraData, frame::Frame, mesh::MeshPushConstants, model::Model,
    object::GpuObjectData, pipeline::Pipeline, render_object::RenderObject,
    scene::GpuSceneData, vertex::Vertex,
};

use super::{
    core::Core, memory::AllocatedBuffer, swapchain::Swapchain, vkinit,
    FRAME_OVERLAP,
};

pub struct Resources {
    pub renderpasses: Vec<Renderpass>,

    pub pipelines: HashMap<String, Rc<Pipeline>>,
    pub models: HashMap<String, Rc<Model>>,

    pub render_objs: ManuallyDrop<Vec<RenderObject>>,
}

impl Resources {
    pub fn new(
        core: &mut Core,
        swapchain: &Swapchain,
        global_desc_set_layout: &vk::DescriptorSetLayout,
        object_desc_set_layout: &vk::DescriptorSetLayout,
    ) -> Result<Self> {
        let device = &core.device;
        let allocator = &mut core.allocator;
        let renderpass = Renderpass::new(device, swapchain)?;

        let pipelines = {
            let pipeline = Rc::new(create_default_pipeline(
                device,
                swapchain,
                &renderpass,
                global_desc_set_layout,
                object_desc_set_layout,
            )?);
            let mut pipelines = HashMap::new();
            pipelines.insert("default".into(), pipeline);
            pipelines
        };
        let models = {
            let monkey_model = Rc::new(Model::load_from_obj(
                "assets/monkey_smooth.obj",
                device,
                allocator,
            )?);
            let triangle_model = Rc::new(Model::new(vec![Mesh::new_triangle(
                device, allocator,
            )?]));
            let mut models = HashMap::new();
            models.insert("monkey".into(), monkey_model);
            models.insert("triangle".into(), triangle_model);
            models
        };
        let render_objs = {
            let mut render_objs = Vec::new();
            let monkey = RenderObject::new(
                Rc::clone(&models["monkey"]),
                Rc::clone(&pipelines["default"]),
                Mat4::IDENTITY,
            );
            render_objs.push(monkey);

            for x in -20..=20 {
                for y in -20..=20 {
                    let translation = Mat4::from_translation(Vec3::new(
                        x as f32, 0.0, y as f32,
                    ));
                    let scale = Mat4::from_scale(Vec3::new(0.2, 0.2, 0.2));
                    let transform = translation * scale;
                    let triangle = RenderObject::new(
                        Rc::clone(&models["triangle"]),
                        Rc::clone(&pipelines["default"]),
                        transform,
                    );
                    render_objs.push(triangle);
                }
            }

            render_objs
        };

        Ok(Self {
            renderpasses: vec![renderpass],
            pipelines,
            models,
            render_objs: ManuallyDrop::new(render_objs),
        })
    }

    pub fn cleanup(mut self, device: &ash::Device, allocator: &mut Allocator) {
        log::info!("Cleaning up assets ...");

        unsafe {
            ManuallyDrop::drop(&mut self.render_objs);
        }

        for (_, model) in self.models {
            if let Ok(model) = Rc::try_unwrap(model) {
                model.cleanup(device, allocator);
            } else {
                panic!("Failed to cleanup model because there are still multiple references");
            }
        }

        for (_, pipeline) in self.pipelines {
            if let Ok(pipeline) = Rc::try_unwrap(pipeline) {
                pipeline.cleanup(device);
            } else {
                panic!("Failed to cleanup pipeline because there are still multiple references");
            }
        }

        for renderpass in self.renderpasses {
            renderpass.cleanup(device);
        }
    }

    pub fn draw_render_objects(
        &self,
        core: &mut Core,
        cmd: &vk::CommandBuffer,
        window: &winit::window::Window,
        first_index: usize,
        count: usize,
        frame: &mut Frame,
        frame_number: u32,
        scene_params_buffer: &mut AllocatedBuffer,
    ) -> Result<()> {
        // Write into camera buffer
        {
            // Fill a GpuCameraData struct
            let cam_pos = Vec3::new(0.0, 6.0, 20.0);
            let view = Mat4::look_to_rh(
                cam_pos,
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 1.0, 0.0),
            );
            let mut proj = Mat4::perspective_rh(
                70.0,
                window.inner_size().width as f32
                    / window.inner_size().height as f32,
                0.1,
                200.0,
            );
            proj.y_axis.y *= -1.0;
            let cam_data = GpuCameraData {
                proj,
                view,
                viewproj: proj * view,
            };

            // Copy GpuCameraData struct to buffer
            let _ = frame.write_to_camera_buffer(&[cam_data])?;
        }

        // Write into object storage buffer
        let object_data = self
            .render_objs
            .iter()
            .map(|obj| obj.transform)
            .collect::<Vec<_>>();
        frame.object_buffer.write(&object_data, 0)?;

        {
            // Fill a GpuSceneData struct
            let framed = frame_number as f32 / 120.0;
            let scene_data = GpuSceneData {
                ambient_color: Vec4::new(framed.sin(), 0.0, framed.cos(), 1.0),
                ..Default::default()
            };

            // Copy GpuSceneData struct to buffer
            let frame_index = frame_number % FRAME_OVERLAP;
            let start_offset = core.pad_uniform_buffer_size(
                std::mem::size_of::<GpuSceneData>() as u64,
            ) * frame_index as u64;
            scene_params_buffer.write(&[scene_data], start_offset as usize)?;
        }

        let mut last_pipeline = vk::Pipeline::null();
        let mut last_model = None;
        for i in first_index..(first_index + count) {
            let device = &core.device;
            let render_obj = &self.render_objs[i];

            // Only bind the pipeline if it doesn't match the already bound one
            if render_obj.pipeline.pipeline != last_pipeline {
                unsafe {
                    device.cmd_bind_pipeline(
                        *cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        render_obj.pipeline.pipeline,
                    );
                }
                last_pipeline = render_obj.pipeline.pipeline;
            }

            let constants = MeshPushConstants {
                data: Vec4::new(0.0, 0.0, 0.0, 0.0),
                render_matrix: render_obj.transform,
            };

            unsafe {
                device.cmd_push_constants(
                    *cmd,
                    render_obj.pipeline.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    bytemuck::bytes_of(&constants),
                );
            }

            // Only bind the mesh if it's a different one from last bind
            let last = last_model.take();
            let model = Some(render_obj.model.clone());
            if model != last {
                // Bind the vertex buffer with offset 0
                let offset = 0;
                unsafe {
                    device.cmd_bind_vertex_buffers(
                        *cmd,
                        0,
                        &[render_obj.model.meshes[0].vertex_buffer.buffer],
                        &[offset],
                    );

                    // Bind global descriptor set
                    let uniform_offset =
                        core.pad_uniform_buffer_size(std::mem::size_of::<
                            GpuSceneData,
                        >()
                            as u64) as u32;
                    device.cmd_bind_descriptor_sets(
                        *cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        render_obj.pipeline.pipeline_layout,
                        0,
                        &[frame.global_desc_set],
                        // Because binding 0 has no dynamic offset, sending 1 offset will affecting binding 1,
                        // which should have a dynamic descriptor.
                        &[uniform_offset],
                    );

                    // Bind object descriptor set
                    device.cmd_bind_descriptor_sets(
                        *cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        render_obj.pipeline.pipeline_layout,
                        1,
                        &[frame.object_desc_set],
                        &[],
                    );
                }
                last_model = model;
            } else {
                last_model = last;
            }

            unsafe {
                device.cmd_draw(
                    *cmd,
                    render_obj.model.meshes[0].vertices.len() as u32,
                    1,
                    0,
                    i as u32,
                );
            }
        }

        Ok(())
    }
}

fn create_default_pipeline(
    device: &ash::Device,
    swapchain: &Swapchain,
    renderpass: &Renderpass,
    global_desc_set_layout: &vk::DescriptorSetLayout,
    object_desc_set_layout: &vk::DescriptorSetLayout,
) -> Result<Pipeline> {
    let mut layout_info = vkinit::pipeline_layout_create_info();

    // Push constants setup
    let push_constant = vk::PushConstantRange {
        offset: 0,
        size: std::mem::size_of::<MeshPushConstants>() as u32,
        stage_flags: vk::ShaderStageFlags::VERTEX,
    };
    layout_info.p_push_constant_ranges = &push_constant;
    layout_info.push_constant_range_count = 1;

    // Descriptor set layout setup
    let set_layouts = [*global_desc_set_layout, *object_desc_set_layout];
    layout_info.set_layout_count = set_layouts.len() as u32;
    layout_info.p_set_layouts = set_layouts.as_ptr();

    let layout = unsafe { device.create_pipeline_layout(&layout_info, None)? };

    let shader = Shader::new("default-lit", device)?;

    let pipeline = PipelineBuilder::new(
        &shader.vert_shader_mod,
        &shader.frag_shader_mod,
        device,
        swapchain,
    )?
    .pipeline_layout(layout, device)
    .vertex_input(Vertex::get_vertex_desc())
    .build(device, renderpass.renderpass)?;

    shader.destroy(device);

    Ok(pipeline)
}

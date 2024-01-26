// Asset initialization
// VkPipeline, VkBuffer, VkImage, VkRenderPass
pub mod mesh;
pub mod model;
pub mod pipeline;
pub mod render_object;
pub mod renderpass;
pub mod shader;
pub mod vertex;

use std::{collections::HashMap, rc::Rc};

use ash::vk;
use glam::{Mat4, Vec3, Vec4};
use gpu_allocator::vulkan::Allocator;
use mesh::Mesh;
use pipeline::PipelineBuilder;
use renderpass::Renderpass;
use shader::Shader;

use self::{
    mesh::MeshPushConstants, model::Model, pipeline::Pipeline,
    render_object::RenderObject, vertex::Vertex,
};

use super::{core::Core, swapchain::Swapchain, vk_initializers};

pub struct Assets {
    pub renderpasses: Vec<Renderpass>,
    pub pipelines: HashMap<String, Rc<Pipeline>>,
    pub models: HashMap<String, Rc<Model>>,
    pub render_objs: Vec<RenderObject>,
}

impl Assets {
    pub fn new(
        core: &mut Core,
        swapchain: &Swapchain,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> {
        let device = &core.device;
        let allocator = &mut core.allocator;

        let renderpass = Renderpass::new(device, swapchain, window)?;
        let pipelines = {
            let pipeline = Rc::new(create_default_pipeline(
                device,
                swapchain,
                &renderpass,
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
                        x as f32,
                        0.0,
                        y as f32,
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
            render_objs,
        })
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        log::info!("Cleaning up assets ...");

        for (_, model) in self.models {
            if let Ok(model) = Rc::try_unwrap(model) {
                model.cleanup(device, allocator);
            }
        }

        for (_, pipeline) in self.pipelines {
            if let Ok(pipeline) = Rc::try_unwrap(pipeline) {
                pipeline.cleanup(device);
            }
        }

        for renderpass in self.renderpasses {
            renderpass.cleanup(device);
        }
    }

    pub fn draw_render_objects(
        &self,
        device: &ash::Device,
        cmd: &vk::CommandBuffer,
        window: &winit::window::Window,
        first_index: usize,
        count: usize,
    ) {
        let cam_pos = Vec3::new(0.0, -6.0, -10.0);
        let view = Mat4::from_translation(cam_pos);
        let proj = Mat4::perspective_rh(
            70.0,
            window.inner_size().width as f32
                / window.inner_size().height as f32,
            0.1,
            200.0,
        );

        let mut last_pipeline = vk::Pipeline::null();
        let mut last_model = None;
        for i in first_index..(first_index+count) {
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

            let model = render_obj.transform;
            let transform = proj * view * model;

            let constants = MeshPushConstants {
                data: Vec4::new(0.0, 0.0, 0.0, 0.0),
                render_matrix: transform,
            };

            // Only bind the mesh if it's a different one from last bind
            let model = render_obj.model.clone();
            if Some(model) != last_model {
                // Bind the vertex buffer with offset 0
                let offset = 0;
                unsafe {
                    device.cmd_bind_vertex_buffers(
                        *cmd,
                        0,
                        &[render_obj.model.meshes[0].vertex_buffer.buffer],
                        &[offset],
                    );
                }
                last_model = Some(model);
            }
            
            unsafe {
                device.cmd_draw(
                    *cmd,
                    render_obj.model.meshes[0].vertices.len() as u32,
                    1,
                    0,
                    0,
                );
            }
        }
    }
}

fn create_default_pipeline(
    device: &ash::Device,
    swapchain: &Swapchain,
    renderpass: &Renderpass,
) -> anyhow::Result<Pipeline> {
    let mut layout_info = vk_initializers::pipeline_layout_create_info();

    let push_constant = vk::PushConstantRange {
        offset: 0,
        size: std::mem::size_of::<MeshPushConstants>() as u32,
        stage_flags: vk::ShaderStageFlags::VERTEX,
    };

    layout_info.p_push_constant_ranges = &push_constant;
    layout_info.push_constant_range_count = 1;

    let layout = unsafe { device.create_pipeline_layout(&layout_info, None)? };

    let shader = Shader::new("tri-mesh", device)?;

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

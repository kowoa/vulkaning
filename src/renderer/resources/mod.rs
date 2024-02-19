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
pub mod texture;
pub mod vertex;

use color_eyre::eyre::Result;
use std::{collections::HashMap, mem::ManuallyDrop, sync::Arc};

use ash::vk;
use glam::{Mat4, Vec3};
use gpu_allocator::vulkan::Allocator;
use mesh::Mesh;
use pipeline::PipelineBuilder;
use renderpass::Renderpass;
use shader::Shader;

use self::{
    mesh::MeshPushConstants, model::Model, pipeline::Pipeline,
    render_object::RenderObject, texture::Texture, vertex::Vertex,
};

use super::{
    core::Core, memory::AllocatedImage, swapchain::Swapchain, vkinit,
    window::Window, UploadContext,
};

pub struct Resources {
    pub renderpasses: Vec<Renderpass>,

    pub pipelines: HashMap<String, Arc<Pipeline>>,
    pub models: HashMap<String, Arc<Model>>,
    pub textures: HashMap<String, Arc<Texture>>,

    pub render_objs: ManuallyDrop<Vec<RenderObject>>,
}

impl Resources {
    pub fn new(
        core: &mut Core,
        swapchain: &Swapchain,
        global_desc_set_layout: &vk::DescriptorSetLayout,
        object_desc_set_layout: &vk::DescriptorSetLayout,
        upload_context: &UploadContext,
        window: &Window,
    ) -> Result<Self> {
        let device = &core.device;
        let mut allocator = core.get_allocator_mut()?;
        let renderpass = Renderpass::new(device, swapchain, window)?;

        let pipelines = {
            let pipeline = Arc::new(create_default_pipeline(
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
            // Create models
            let mut monkey_model = Model::load_from_obj(
                "monkey_smooth.obj",
                device,
                &mut allocator,
            )?;
            let mut triangle_model = Model::new(vec![Mesh::new_triangle()]);

            // Upload models onto GPU immediately
            monkey_model.meshes[0].upload(
                device,
                &mut allocator,
                upload_context,
            )?;
            triangle_model.meshes[0].upload(
                device,
                &mut allocator,
                upload_context,
            )?;

            // Create HashMap with model name as keys and model as values
            let mut models = HashMap::new();
            models.insert("monkey".into(), Arc::new(monkey_model));
            models.insert("triangle".into(), Arc::new(triangle_model));
            models
        };

        let textures = {
            let image = AllocatedImage::load_from_file(
                "lost_empire-RGBA.png",
                device,
                &mut allocator,
                upload_context,
            )?;

            let mut textures = HashMap::new();
            textures.insert(
                "empire_diffuse".to_string(),
                Arc::new(Texture { image }),
            );
            textures
        };

        let render_objs = {
            let mut render_objs = Vec::new();
            let monkey = RenderObject::new(
                Arc::clone(&models["monkey"]),
                Arc::clone(&pipelines["default"]),
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
                        Arc::clone(&models["triangle"]),
                        Arc::clone(&pipelines["default"]),
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
            textures,
            render_objs: ManuallyDrop::new(render_objs),
        })
    }

    pub fn cleanup(mut self, device: &ash::Device, allocator: &mut Allocator) {
        log::info!("Cleaning up assets ...");

        unsafe {
            ManuallyDrop::drop(&mut self.render_objs);
        }

        for (_, model) in self.models {
            if let Ok(model) = Arc::try_unwrap(model) {
                model.cleanup(device, allocator);
            } else {
                panic!("Failed to cleanup model because there are still multiple references");
            }
        }

        for (_, pipeline) in self.pipelines {
            if let Ok(pipeline) = Arc::try_unwrap(pipeline) {
                pipeline.cleanup(device);
            } else {
                panic!("Failed to cleanup pipeline because there are still multiple references");
            }
        }

        for renderpass in self.renderpasses {
            renderpass.cleanup(device);
        }
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

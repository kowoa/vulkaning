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

use color_eyre::eyre::{OptionExt, Result};
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

        let pipelines = Self::create_pipelines(
            device,
            swapchain,
            &renderpass,
            global_desc_set_layout,
            object_desc_set_layout,
        )?;

        let models = {
            // Create models
            let mut monkey_model = Model::load_from_obj("monkey_smooth.obj")?;
            let mut triangle_model = Model::new(vec![Mesh::new_triangle()]);
            let mut empire_model = Model::load_from_obj("lost_empire.obj")?;
            let mut backpack_model = Model::load_from_obj("backpack.obj")?;

            // Upload models onto GPU immediately
            monkey_model.upload(device, &mut allocator, upload_context)?;
            triangle_model.upload(device, &mut allocator, upload_context)?;
            empire_model.upload(device, &mut allocator, upload_context)?;
            backpack_model.upload(device, &mut allocator, &upload_context)?;

            // Create HashMap with model name as keys and model as values
            let mut models = HashMap::new();
            models.insert("monkey".into(), Arc::new(monkey_model));
            models.insert("triangle".into(), Arc::new(triangle_model));
            models.insert("empire".into(), Arc::new(empire_model));
            models.insert("backpack".into(), Arc::new(backpack_model));
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
                Arc::clone(&pipelines["default-lit"]),
                Mat4::IDENTITY,
            );
            render_objs.push(monkey);

            /*
            for x in -20..=20 {
                for y in -20..=20 {
                    let translation = Mat4::from_translation(Vec3::new(
                        x as f32, 0.0, y as f32,
                    ));
                    let scale = Mat4::from_scale(Vec3::new(0.2, 0.2, 0.2));
                    let transform = translation * scale;
                    let triangle = RenderObject::new(
                        Arc::clone(&models["triangle"]),
                        Arc::clone(&pipelines["default-lit"]),
                        transform,
                    );
                    render_objs.push(triangle);
                }
            }
            */

            let empire = RenderObject::new(
                models["empire"].clone(),
                pipelines["default-lit"].clone(),
                Mat4::IDENTITY,
            );
            //render_objs.push(empire);

            let backpack = RenderObject::new(
                models["backpack"].clone(),
                pipelines["default-lit"].clone(),
                Mat4::IDENTITY,
            );
            render_objs.push(backpack);

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

    /*
    fn create_scene(
        &self,
        device: &ash::Device,
        pipelines: &HashMap<String, Arc<Pipeline>>,
        descriptor_pool: &vk::DescriptorPool,
    ) -> Result<()> {
        // Create sampler for the texture
        let sampler_info = vkinit::sampler_create_info(
            vk::Filter::NEAREST,
            vk::SamplerAddressMode::REPEAT,
        );
        let blocky_sampler =
            unsafe { device.create_sampler(&sampler_info, None)? };

        let textured_lit_pipeline = self
            .pipelines
            .get("textured-lit")
            .ok_or_eyre("textured-lit pipeline not found")?;

        // Allocate the descriptor set for the single-texture to use on the material
        let alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: *descriptor_pool,
            descriptor_set_count: 1,
            p_set_layouts: &
        };

        Ok(())
    }
    */

    fn create_pipelines(
        device: &ash::Device,
        swapchain: &Swapchain,
        renderpass: &Renderpass,
        global_desc_set_layout: &vk::DescriptorSetLayout,
        object_desc_set_layout: &vk::DescriptorSetLayout,
    ) -> Result<HashMap<String, Arc<Pipeline>>> {
        let pipeline_layout = {
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
            let set_layouts =
                [*global_desc_set_layout, *object_desc_set_layout];
            layout_info.set_layout_count = set_layouts.len() as u32;
            layout_info.p_set_layouts = set_layouts.as_ptr();

            // Create pipeline layout
            unsafe { device.create_pipeline_layout(&layout_info, None)? }
        };

        let default_lit_pipeline = {
            let default_lit_shader = Shader::new("default-lit", device)?;
            let default_lit_pipeline = PipelineBuilder::new(
                &default_lit_shader.vert_shader_mod,
                &default_lit_shader.frag_shader_mod,
                device,
                swapchain,
            )?
            .pipeline_layout(pipeline_layout)
            .vertex_input(Vertex::get_vertex_desc())
            .build(device, renderpass.renderpass)?;
            default_lit_shader.cleanup(device);
            default_lit_pipeline
        };

        let textured_lit_pipeline = {
            let textured_lit_shader = Shader::new("textured-lit", device)?;
            let textured_lit_pipeline = PipelineBuilder::new(
                &textured_lit_shader.vert_shader_mod,
                &textured_lit_shader.frag_shader_mod,
                device,
                swapchain,
            )?
            .pipeline_layout(pipeline_layout)
            .vertex_input(Vertex::get_vertex_desc())
            .build(device, renderpass.renderpass)?;
            textured_lit_shader.cleanup(device);
            textured_lit_pipeline
        };

        let mut map = HashMap::new();
        map.insert("default-lit".into(), Arc::new(default_lit_pipeline));
        map.insert("textured-lit".into(), Arc::new(textured_lit_pipeline));
        Ok(map)
    }
}

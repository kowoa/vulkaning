// Asset initialization
pub mod camera;
pub mod frame;
pub mod material;
pub mod mesh;
pub mod model;
pub mod object;
pub mod render_object;
pub mod renderpass;
pub mod scene;
pub mod texture;
pub mod vertex;

use color_eyre::eyre::Result;
use std::{collections::HashMap, mem::ManuallyDrop, sync::Arc};

use ash::vk;
use glam::{Mat4, Vec3};
use gpu_allocator::vulkan::Allocator;
use mesh::Mesh;
use renderpass::Renderpass;

use self::{
    material::{Material, MaterialBuilder},
    mesh::MeshPushConstants,
    model::Model,
    render_object::RenderObject,
    texture::Texture,
    vertex::Vertex,
};

use super::{
    core::Core,
    descriptors::DescriptorAllocator,
    shader::{ComputeShader, Shader},
    swapchain::Swapchain,
    vkinit, UploadContext,
};

pub struct Resources {
    pub renderpasses: Vec<Renderpass>,

    pub materials: HashMap<String, Arc<Material>>,
    pub models: HashMap<String, Arc<Model>>,
    pub textures: HashMap<String, Arc<Texture>>,

    pub render_objs: ManuallyDrop<Vec<RenderObject>>,
}

impl Resources {
    pub fn new(
        core: &mut Core,
        swapchain: &Swapchain,
        upload_context: &UploadContext,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<Self> {
        let mut allocator = core.get_allocator()?;

        let renderpass = Renderpass::new(&core.device, swapchain)?;

        let materials = Self::create_materials(
            &core.device,
            swapchain,
            &renderpass,
            desc_allocator,
        )?;

        let models = {
            // Create models
            let mut monkey_model = Model::load_from_obj("monkey_smooth.obj")?;
            let mut triangle_model = Model::new(vec![Mesh::new_triangle()]);
            let mut empire_model = Model::load_from_obj("lost_empire.obj")?;
            let mut backpack_model = Model::load_from_obj("backpack.obj")?;

            // Upload models onto GPU immediately
            {
                monkey_model.upload(
                    &core.device,
                    &mut allocator,
                    upload_context,
                )?;
                triangle_model.upload(
                    &core.device,
                    &mut allocator,
                    upload_context,
                )?;
                empire_model.upload(
                    &core.device,
                    &mut allocator,
                    upload_context,
                )?;
                backpack_model.upload(
                    &core.device,
                    &mut allocator,
                    upload_context,
                )?;
            }

            // Create HashMap with model name as keys and model as values
            let mut models = HashMap::new();
            models.insert("monkey".into(), Arc::new(monkey_model));
            models.insert("triangle".into(), Arc::new(triangle_model));
            models.insert("empire".into(), Arc::new(empire_model));
            models.insert("backpack".into(), Arc::new(backpack_model));
            models
        };

        let textures = {
            let empire = Texture::load_from_file(
                "lost_empire-RGBA.png",
                &core.device,
                &mut allocator,
                desc_allocator,
                upload_context,
            )?;

            let mut textures = HashMap::new();
            textures.insert("empire-diffuse".to_string(), Arc::new(empire));
            textures
        };

        // Scene/render objects
        let render_objs = {
            let mut render_objs = Vec::new();
            let monkey = RenderObject::new(
                models["monkey"].clone(),
                materials["default-lit"].clone(),
                None,
                Mat4::from_translation(Vec3::new(0.0, 20.0, -20.0)),
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
                materials["textured-lit"].clone(),
                Some(textures["empire-diffuse"].clone()),
                Mat4::IDENTITY,
            );
            //render_objs.push(empire);

            /*
            let backpack = RenderObject::new(
                models["backpack"].clone(),
                pipelines["default-lit"].clone(),
                Mat4::IDENTITY,
            );
            */
            //render_objs.push(backpack);

            render_objs
        };

        Ok(Self {
            renderpasses: vec![renderpass],
            materials,
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

        for (_, material) in self.materials {
            if let Ok(material) = Arc::try_unwrap(material) {
                material.cleanup(device);
            } else {
                panic!("Failed to cleanup pipeline because there are still multiple references");
            }
        }

        for (_, texture) in self.textures {
            if let Ok(texture) = Arc::try_unwrap(texture) {
                texture.cleanup(device, allocator);
            }
        }

        for renderpass in self.renderpasses {
            renderpass.cleanup(device);
        }
    }

    fn create_materials(
        device: &ash::Device,
        swapchain: &Swapchain,
        renderpass: &Renderpass,
        desc_allocator: &DescriptorAllocator,
    ) -> Result<HashMap<String, Arc<Material>>> {
        let global_desc_set_layout = desc_allocator.get_layout("global")?;
        let object_desc_set_layout = desc_allocator.get_layout("object")?;
        let single_texture_desc_set_layout =
            desc_allocator.get_layout("single texture")?;
        let draw_image_desc_set_layout =
            desc_allocator.get_layout("draw image")?;

        let default_lit_mat = {
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

            let default_lit_shader = Shader::new("default-lit", device)?;
            let default_lit_mat = MaterialBuilder::new(
                &default_lit_shader.vert_shader_mod,
                &default_lit_shader.frag_shader_mod,
                device,
                swapchain,
            )?
            .pipeline_layout(pipeline_layout)
            .vertex_input(Vertex::get_vertex_desc())
            .build(device, renderpass.renderpass)?;
            default_lit_shader.cleanup(device);
            default_lit_mat
        };

        let textured_lit_mat = {
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
                let set_layouts = [
                    *global_desc_set_layout,
                    *object_desc_set_layout,
                    *single_texture_desc_set_layout,
                ];
                layout_info.set_layout_count = set_layouts.len() as u32;
                layout_info.p_set_layouts = set_layouts.as_ptr();
                // Create pipeline layout
                unsafe { device.create_pipeline_layout(&layout_info, None)? }
            };
            let textured_lit_shader = Shader::new("textured-lit", device)?;
            let textured_lit_mat = MaterialBuilder::new(
                &textured_lit_shader.vert_shader_mod,
                &textured_lit_shader.frag_shader_mod,
                device,
                swapchain,
            )?
            .pipeline_layout(pipeline_layout)
            .vertex_input(Vertex::get_vertex_desc())
            .build(device, renderpass.renderpass)?;
            textured_lit_shader.cleanup(device);
            textured_lit_mat
        };

        let gradient_mat = {
            //let pipeline_layout = vk::PipelineLayoutCreateInfo::builder();
            let gradient_shader = ComputeShader::new("gradient", device)?;
            gradient_shader.cleanup(device);
        };

        let mut map = HashMap::new();
        map.insert("default-lit".into(), Arc::new(default_lit_mat));
        map.insert("textured-lit".into(), Arc::new(textured_lit_mat));
        Ok(map)
    }
}

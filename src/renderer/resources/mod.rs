pub mod camera;
pub mod frame;
pub mod mesh;
pub mod model;
pub mod object;
pub mod render_object;
pub mod renderpass;
pub mod scene;
pub mod texture;
pub mod vertex;

use color_eyre::eyre::{eyre, Result};
use std::{collections::HashMap, ffi::CString, mem::ManuallyDrop, sync::Arc};

use ash::vk;
use glam::{Mat4, Vec3, Vec4};
use gpu_allocator::vulkan::Allocator;
use mesh::Mesh;
use renderpass::Renderpass;

use self::{
    mesh::MeshPushConstants, model::Model, render_object::RenderObject,
    texture::Texture, vertex::Vertex,
};

use super::{
    core::Core,
    descriptors::DescriptorAllocator,
    material::Material,
    shader::{ComputeEffect, ComputePushConstants, ComputeShader, Shader},
    swapchain::Swapchain,
    upload_context::UploadContext,
    vkinit,
};

pub struct Resources {
    pub renderpass: Renderpass,

    materials: HashMap<String, Arc<Material>>,
    models: HashMap<String, Arc<Model>>,
    textures: HashMap<String, Arc<Texture>>,

    pub render_objs: ManuallyDrop<Vec<RenderObject>>,
    pub background_effects: Vec<ComputeEffect>,
    pub current_background_effects_index: usize,
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

        let mut background_effects = Vec::new();
        let materials = Self::create_materials(
            &core.device,
            swapchain,
            &renderpass,
            desc_allocator,
            &mut background_effects,
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
            renderpass,
            materials,
            models,
            textures,
            render_objs: ManuallyDrop::new(render_objs),
            background_effects,
            current_background_effects_index: 0,
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

        self.renderpass.cleanup(device);
    }

    fn create_materials(
        device: &ash::Device,
        swapchain: &Swapchain,
        renderpass: &Renderpass,
        desc_allocator: &DescriptorAllocator,
        background_fx: &mut Vec<ComputeEffect>,
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
            let default_lit_mat = Material::builder(
                &default_lit_shader.vert_shader_mod,
                &default_lit_shader.frag_shader_mod,
                device,
                swapchain,
            )
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
            let textured_lit_mat = Material::builder(
                &textured_lit_shader.vert_shader_mod,
                &textured_lit_shader.frag_shader_mod,
                device,
                swapchain,
            )
            .pipeline_layout(pipeline_layout)
            .vertex_input(Vertex::get_vertex_desc())
            .build(device, renderpass.renderpass)?;
            textured_lit_shader.cleanup(device);
            textured_lit_mat
        };

        let gradient_mat = {
            let pipeline_layout = {
                let layouts = [*draw_image_desc_set_layout];
                let push_constant_ranges = [vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::COMPUTE,
                    offset: 0,
                    size: std::mem::size_of::<ComputePushConstants>() as u32,
                }];
                let layout_info = vk::PipelineLayoutCreateInfo::builder()
                    .set_layouts(&layouts)
                    .push_constant_ranges(&push_constant_ranges)
                    .build();
                unsafe { device.create_pipeline_layout(&layout_info, None)? }
            };
            let gradient_shader = ComputeShader::new("gradient-color", device)?;
            let name = CString::new("main")?;
            let stage_info = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(gradient_shader.shader_mod)
                .name(&name)
                .build();
            let pipeline_info = vk::ComputePipelineCreateInfo::builder()
                .layout(pipeline_layout)
                .stage(stage_info)
                .build();
            let gradient_pipeline = unsafe {
                match device.create_compute_pipelines(
                    vk::PipelineCache::null(),
                    &[pipeline_info],
                    None,
                ) {
                    Ok(pipelines) => Ok(pipelines),
                    Err(_) => Err(eyre!("Failed to create compute pipelines")),
                }
            }?[0];
            gradient_shader.cleanup(device);
            Material::new(gradient_pipeline, pipeline_layout)
        };
        let gradient_comp_fx = ComputeEffect {
            name: "gradient".into(),
            material: gradient_mat.clone(),
            data: ComputePushConstants {
                data1: Vec4::new(1.0, 0.0, 0.0, 1.0),
                data2: Vec4::new(0.0, 0.0, 1.0, 1.0),
                ..Default::default()
            },
        };

        let sky_mat = {
            let pipeline_layout = {
                let layouts = [*draw_image_desc_set_layout];
                let push_constant_ranges = [vk::PushConstantRange {
                    stage_flags: vk::ShaderStageFlags::COMPUTE,
                    offset: 0,
                    size: std::mem::size_of::<ComputePushConstants>() as u32,
                }];
                let layout_info = vk::PipelineLayoutCreateInfo::builder()
                    .set_layouts(&layouts)
                    .push_constant_ranges(&push_constant_ranges)
                    .build();
                unsafe { device.create_pipeline_layout(&layout_info, None)? }
            };
            let sky_shader = ComputeShader::new("sky", device)?;
            let name = CString::new("main")?;
            let stage_info = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(sky_shader.shader_mod)
                .name(&name)
                .build();
            let pipeline_info = vk::ComputePipelineCreateInfo::builder()
                .layout(pipeline_layout)
                .stage(stage_info)
                .build();
            let sky_pipeline = unsafe {
                match device.create_compute_pipelines(
                    vk::PipelineCache::null(),
                    &[pipeline_info],
                    None,
                ) {
                    Ok(pipelines) => Ok(pipelines),
                    Err(_) => Err(eyre!("Failed to create compute pipelines")),
                }
            }?[0];
            sky_shader.cleanup(device);
            Material::new(sky_pipeline, pipeline_layout)
        };
        let sky_comp_fx = ComputeEffect {
            name: "sky".into(),
            material: sky_mat.clone(),
            data: ComputePushConstants {
                data1: Vec4::new(0.1, 0.2, 0.4, 0.97),
                ..Default::default()
            },
        };

        background_fx.push(gradient_comp_fx);
        background_fx.push(sky_comp_fx);

        let mut map = HashMap::new();
        map.insert("default-lit".into(), Arc::new(default_lit_mat));
        map.insert("textured-lit".into(), Arc::new(textured_lit_mat));
        map.insert("gradient".into(), Arc::new(gradient_mat));
        map.insert("sky".into(), Arc::new(sky_mat));
        Ok(map)
    }
}

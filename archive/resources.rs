use bevy::log;
pub mod object;
pub mod renderpass;
pub mod scene;

use color_eyre::eyre::Result;
use std::{collections::HashMap, mem::ManuallyDrop, sync::Arc};

use ash::vk;
use glam::{Mat4, Vec3, Vec4};
use gpu_allocator::vulkan::Allocator;

use super::{
    core::Core,
    descriptors::DescriptorAllocator,
    image::AllocatedImage,
    material::Material,
    mesh::{Mesh, MeshPushConstants},
    model::Model,
    render_object::RenderObject,
    shader::{
        ComputeEffect, ComputePushConstants, ComputeShader, GraphicsShader,
    },
    swapchain::Swapchain,
    texture::Texture,
    upload_context::UploadContext,
    vertex::Vertex,
    vkinit,
};

pub struct Resources {
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
        draw_image: &AllocatedImage,
    ) -> Result<Self> {
        let mut allocator = core.get_allocator()?;

        let mut background_effects = Vec::new();
        let materials = Self::create_materials(
            &core.device,
            swapchain,
            desc_allocator,
            &mut background_effects,
            draw_image,
        )?;

        let models = {
            // Create models
            //let mut monkey_model = Model::load_from_obj("monkey_smooth.obj")?;
            //let mut triangle_model = Model::new(vec![Mesh::new_triangle()]);
            //let mut empire_model = Model::load_from_obj("lost_empire.obj")?;
            let mut backpack_model =
                Model::load_from_obj("backpack/backpack.obj")?;
            let mut quad_model = Model::new(vec![Mesh::new_quad()]);

            // Upload models onto GPU immediately
            {
                /*
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
                */
                backpack_model.upload(
                    &core.device,
                    &mut allocator,
                    upload_context,
                )?;
                quad_model.upload(
                    &core.device,
                    &mut allocator,
                    upload_context,
                )?;
            }

            // Create HashMap with model name as keys and model as values
            let mut models = HashMap::new();
            //models.insert("monkey".into(), Arc::new(monkey_model));
            //models.insert("triangle".into(), Arc::new(triangle_model));
            //models.insert("empire".into(), Arc::new(empire_model));
            models.insert("backpack".into(), Arc::new(backpack_model));
            models.insert("quad".into(), Arc::new(quad_model));
            models
        };

        let textures = {
            /*
            let empire = Texture::load_from_file(
                "lost_empire-RGBA.png",
                false,
                &core.device,
                &mut allocator,
                desc_allocator,
                upload_context,
            )?;
            */
            let backpack = Texture::load_from_file(
                "backpack/diffuse.jpg",
                true,
                &core.device,
                &mut allocator,
                desc_allocator,
                upload_context,
            )?;

            let mut textures = HashMap::new();
            //textures.insert("empire-diffuse".into(), Arc::new(empire));
            textures.insert("backpack-diffuse".into(), Arc::new(backpack));
            textures
        };

        // Scene/render objects
        let render_objs = {
            let mut render_objs = Vec::new();

            let quad = RenderObject::new(
                models["quad"].clone(),
                materials["default-lit"].clone(),
                None,
                Mat4::from_rotation_x(90f32.to_radians()),
            );
            render_objs.push(quad);

            /*
            let empire = RenderObject::new(
                models["empire"].clone(),
                materials["textured-lit"].clone(),
                Some(textures["empire-diffuse"].clone()),
                Mat4::IDENTITY,
            );
            */
            //render_objs.push(empire);

            let backpack = RenderObject::new(
                models["backpack"].clone(),
                materials["textured-lit"].clone(),
                Some(textures["backpack-diffuse"].clone()),
                Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0)),
            );
            render_objs.push(backpack);

            render_objs
        };

        Ok(Self {
            materials,
            models,
            textures,
            render_objs: ManuallyDrop::new(render_objs),
            background_effects,
            current_background_effects_index: 1,
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
    }
}

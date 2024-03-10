use bevy::log;
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use std::{
    collections::HashMap,
    mem::ManuallyDrop,
    sync::{Arc, Mutex, MutexGuard},
};

use ash::vk;
use color_eyre::eyre::{eyre, Result};

use super::{
    camera::Camera,
    context::Context,
    descriptors::DescriptorSetLayoutBuilder,
    frame::Frame,
    material::Material,
    mesh::Mesh,
    model::Model,
    render_resources::RenderResources,
    shader::GraphicsShader,
    swapchain::Swapchain,
    texture::{Texture, TextureAssetData},
    AssetData,
};

pub const FRAME_OVERLAP: u32 = 2;
pub const MAX_OBJECTS: u32 = 10000; // Max objects per frame

pub struct DrawContext<'a> {
    pub context: Arc<Context>,
    pub swapchain: Arc<Swapchain>,
    pub resources: Arc<Mutex<RenderResources>>,

    pub frame_number: u32,
    pub camera: &'a Camera,
    pub background_texture: Arc<Mutex<Texture>>,
}

pub struct RendererInner {
    context: Arc<Context>,
    swapchain: Arc<Swapchain>,
    allocator: ManuallyDrop<Arc<Mutex<Allocator>>>,
    resources: Arc<Mutex<RenderResources>>,

    frame_number: u32,
    frames: Vec<Frame>,
    command_pool: vk::CommandPool,

    background_texture: Arc<Mutex<Texture>>,
}

impl RendererInner {
    pub fn new(window: &winit::window::Window) -> Result<Self> {
        log::info!("Initializing renderer ...");

        let mut ctx = Context::new(window)?;
        let mut allocator = Allocator::new(&AllocatorCreateDesc {
            instance: ctx.instance.clone(),
            device: ctx.device.clone(),
            physical_device: ctx.physical_device,
            debug_settings: AllocatorDebugSettings {
                log_memory_information: true,
                log_leaks_on_shutdown: true,
                store_stack_traces: false,
                log_allocations: true,
                log_frees: true,
                log_stack_traces: false,
            },
            buffer_device_address: true,
            allocation_sizes: Default::default(),
        })?;
        let swapchain = Swapchain::new(&ctx, &mut allocator, window)?;

        let mut resources = RenderResources::default();
        Self::init_desc_set_layouts(
            &ctx.device,
            &mut resources.desc_set_layouts,
        )?;

        let command_pool =
            Self::create_command_pool(&ctx.device, ctx.graphics_queue_family)?;

        let frames = {
            let mut frames = Vec::with_capacity(FRAME_OVERLAP as usize);
            for _ in 0..FRAME_OVERLAP {
                // Call Frame constructor
                frames.push(Frame::new(
                    &mut ctx,
                    &mut allocator,
                    &command_pool,
                )?);
            }
            frames
        };

        let background_texture = Texture::new_compute_texture(
            swapchain.image_extent.width,
            swapchain.image_extent.height,
            &ctx.device,
            &mut allocator,
        )?;

        Ok(Self {
            context: Arc::new(ctx),
            swapchain: Arc::new(swapchain),
            allocator: ManuallyDrop::new(Arc::new(Mutex::new(allocator))),
            frame_number: 0,
            frames,
            command_pool,
            resources: Arc::new(Mutex::new(resources)),
            background_texture: Arc::new(Mutex::new(background_texture)),
        })
    }

    pub fn init_resources(&mut self, assets: &mut AssetData) -> Result<()> {
        self.init_models(&mut assets.models)?;
        self.init_textures(&mut assets.textures)?;
        self.init_materials()?;

        Ok(())
    }

    pub fn draw_frame(&mut self, camera: &Camera) -> Result<()> {
        let ctx = DrawContext {
            context: self.context.clone(),
            swapchain: self.swapchain.clone(),
            resources: self.resources.clone(),
            frame_number: self.frame_number,
            camera,
            background_texture: self.background_texture.clone(),
        };
        self.get_current_frame().draw(ctx)?;
        self.frame_number += 1;

        Ok(())
    }

    pub fn cleanup(mut self) {
        // Wait until all frames have finished rendering
        for frame in &self.frames {
            unsafe {
                self.context
                    .device
                    .wait_for_fences(&[frame.render_fence()], true, 1000000000)
                    .unwrap();
            }
        }

        {
            let device = &self.context.device;
            let mut allocator = self.allocator.lock().unwrap();

            match Arc::try_unwrap(self.resources) {
                Ok(resources) => {
                    resources
                        .into_inner()
                        .unwrap()
                        .cleanup(device, &mut allocator);
                    Ok(())
                }
                Err(_) => Err(eyre!("Failed to cleanup resources")),
            }
            .unwrap();

            // Destroy command pool
            unsafe {
                device.destroy_command_pool(self.command_pool, None);
            }

            // Clean up all frames
            for frame in self.frames.drain(..) {
                frame.cleanup(device, &mut allocator);
            }

            // Clean up background texture
            match Arc::try_unwrap(self.background_texture) {
                Ok(texture) => {
                    texture
                        .into_inner()
                        .unwrap()
                        .cleanup(device, &mut allocator);
                    Ok(())
                }
                Err(_) => Err(eyre!("Failed to cleanup background texture")),
            }
            .unwrap();

            // Clean up swapchain
            match Arc::try_unwrap(self.swapchain) {
                Ok(swapchain) => {
                    swapchain.cleanup(device, &mut allocator);
                    Ok(())
                }
                Err(_) => Err(eyre!("Failed to cleanup swapchain")),
            }
            .unwrap()
        }
        // We need to do this because the allocator doesn't destroy all
        // memory blocks (VkDeviceMemory) until it is dropped.
        unsafe { ManuallyDrop::drop(&mut self.allocator) };

        // Clean up core Vulkan objects
        match Arc::try_unwrap(self.context) {
            Ok(context) => context.cleanup(),
            Err(_) => log::error!("Failed to cleanup context"),
        }
    }

    fn get_current_frame(&mut self) -> &mut Frame {
        &mut self.frames[(self.frame_number % FRAME_OVERLAP) as usize]
    }

    fn get_allocator(&self) -> Result<MutexGuard<Allocator>> {
        match self.allocator.lock() {
            Ok(allocator) => Ok(allocator),
            Err(err) => Err(eyre!(err.to_string())),
        }
    }

    fn get_resources(&self) -> Result<MutexGuard<RenderResources>> {
        match self.resources.lock() {
            Ok(resources) => Ok(resources),
            Err(err) => Err(eyre!(err.to_string())),
        }
    }

    /// Helper function that creates a command pool
    fn create_command_pool(
        device: &ash::Device,
        graphics_family_index: u32,
    ) -> Result<vk::CommandPool> {
        let pool_info = vk::CommandPoolCreateInfo {
            queue_family_index: graphics_family_index,
            // Allow the pool to reset individual command buffers
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            ..Default::default()
        };
        let command_pool =
            unsafe { device.create_command_pool(&pool_info, None)? };
        Ok(command_pool)
    }

    fn init_desc_set_layouts(
        device: &ash::Device,
        desc_set_layouts: &mut HashMap<String, vk::DescriptorSetLayout>,
    ) -> Result<()> {
        let compute_texture_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::STORAGE_IMAGE,
                vk::ShaderStageFlags::COMPUTE,
            )
            .build(device)?;
        desc_set_layouts
            .insert("compute texture".into(), compute_texture_layout);

        let graphics_texture_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::FRAGMENT,
            )
            .build(device)?;
        desc_set_layouts
            .insert("graphics texture".into(), graphics_texture_layout);

        let scene_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::UNIFORM_BUFFER,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .build(device)?;
        desc_set_layouts.insert("scene buffer".into(), scene_layout);

        Ok(())
    }

    /// Upload all models to the GPU
    fn init_models(
        &mut self,
        models: &mut HashMap<String, Model>,
    ) -> Result<()> {
        let mut resources = self.get_resources()?;

        // Upload asset models to the GPU
        for (name, mut model) in models.drain() {
            model.upload(&self.context, &mut *self.get_allocator()?)?;
            resources.models.insert(name, model);
        }
        // Upload other models to the GPU
        let quad = Mesh::new_quad();
        let mut quad = Model::new(vec![quad]);
        quad.upload(&self.context, &mut *self.get_allocator()?)?;
        resources.models.insert("quad".into(), quad);

        Ok(())
    }

    fn init_textures(
        &mut self,
        textures: &mut HashMap<String, TextureAssetData>,
    ) -> Result<()> {
        let mut resources = self.get_resources()?;

        for (name, data) in textures.drain() {
            if !resources.samplers.contains_key(&data.filter) {
                resources.create_sampler(data.filter, &self.context.device)?;
            }
            let sampler = resources.samplers[&data.filter];
            let texture = Texture::new_graphics_texture(
                data,
                sampler,
                &self.context,
                &mut *self.get_allocator()?,
            )?;
            resources.textures.insert(name, texture);
        }

        Ok(())
    }

    /// Create materials and insert them into RenderResources
    fn init_materials(&mut self) -> Result<()> {
        let mut resources = self.get_resources()?;

        let scene_buffer_layout = resources.desc_set_layouts["scene buffer"];
        let graphics_texture_layout =
            resources.desc_set_layouts["graphics texture"];
        #[allow(unused_variables)]
        let compute_texture_layout =
            resources.desc_set_layouts["compute texture"];

        let default_mat = {
            let set_layouts = [scene_buffer_layout];
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&set_layouts)
                .build();
            let pipeline_layout = unsafe {
                self.context
                    .device
                    .create_pipeline_layout(&pipeline_layout_info, None)?
            };
            Material::builder_graphics(&self.context.device)
                .pipeline_layout(pipeline_layout)
                .shader(GraphicsShader::new("default", &self.context.device)?)
                .color_attachment_format(self.swapchain.image_format)
                .depth_attachment_format(self.swapchain.depth_image.format)
                .build()?
        };
        resources.materials.insert("default".into(), default_mat);

        let grid_mat = {
            let set_layouts = [scene_buffer_layout];
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&set_layouts)
                .build();
            let pipeline_layout = unsafe {
                self.context
                    .device
                    .create_pipeline_layout(&pipeline_layout_info, None)?
            };
            Material::builder_graphics(&self.context.device)
                .pipeline_layout(pipeline_layout)
                .shader(GraphicsShader::new("grid", &self.context.device)?)
                .color_attachment_format(self.swapchain.image_format)
                .depth_attachment_format(self.swapchain.depth_image.format)
                .build()?
        };
        resources.materials.insert("grid".into(), grid_mat);

        let textured_mat = {
            let set_layouts = [scene_buffer_layout, graphics_texture_layout];
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&set_layouts)
                .build();
            let pipeline_layout = unsafe {
                self.context
                    .device
                    .create_pipeline_layout(&pipeline_layout_info, None)?
            };
            Material::builder_graphics(&self.context.device)
                .pipeline_layout(pipeline_layout)
                .shader(GraphicsShader::new("textured", &self.context.device)?)
                .color_attachment_format(self.swapchain.image_format)
                .depth_attachment_format(self.swapchain.depth_image.format)
                .build()?
        };
        resources.materials.insert("textured".into(), textured_mat);

        Ok(())
    }

    /*
    fn create_materials(
        device: &ash::Device,
        swapchain: &Swapchain,
        desc_allocator: &DescriptorAllocator,
        background_fx: &mut Vec<ComputeEffect>,
        draw_image: &AllocatedImage,
    ) -> Result<HashMap<String, Material>> {
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

            let default_lit_shader =
                GraphicsShader::new("default-lit", device)?;
            Material::builder_graphics(device)
                .pipeline_layout(pipeline_layout)
                .shader(default_lit_shader)
                .vertex_input(Vertex::get_vertex_desc())
                .color_attachment_format(draw_image.format)
                .depth_attachment_format(swapchain.depth_image.format)
                .build()?
        };

        let default_mat = {
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
                let set_layouts = [*global_desc_set_layout];
                layout_info.set_layout_count = set_layouts.len() as u32;
                layout_info.p_set_layouts = set_layouts.as_ptr();

                // Create pipeline layout
                unsafe { device.create_pipeline_layout(&layout_info, None)? }
            };

            let default_lit_shader = GraphicsShader::new("default", device)?;
            Material::builder_graphics(device)
                .pipeline_layout(pipeline_layout)
                .shader(default_lit_shader)
                .vertex_input(Vertex::get_vertex_desc())
                .color_attachment_format(draw_image.format)
                .depth_attachment_format(swapchain.depth_image.format)
                .build()?
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
            let textured_lit_shader =
                GraphicsShader::new("textured-lit", device)?;
            Material::builder_graphics(device)
                .pipeline_layout(pipeline_layout)
                .shader(textured_lit_shader)
                .vertex_input(Vertex::get_vertex_desc())
                .color_attachment_format(draw_image.format)
                .depth_attachment_format(swapchain.depth_image.format)
                .build()?
        };

        let grid_mat = {
            let push_constant_ranges = [vk::PushConstantRange::builder()
                .offset(0)
                .size(std::mem::size_of::<Mat4>() as u32)
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .build()];
            let set_layouts = [*global_desc_set_layout];
            let layout_info = vk::PipelineLayoutCreateInfo::builder()
                .push_constant_ranges(&push_constant_ranges)
                .set_layouts(&set_layouts)
                .build();
            // Create pipeline layout
            let layout =
                unsafe { device.create_pipeline_layout(&layout_info, None)? };
            let shader = GraphicsShader::new("grid", device)?;
            Material::builder_graphics(device)
                .pipeline_layout(layout)
                .shader(shader)
                .color_attachment_format(draw_image.format)
                .depth_attachment_format(swapchain.depth_image.format)
                .build()?
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
            Material::builder_compute(device)
                .pipeline_layout(pipeline_layout)
                .shader(gradient_shader)
                .build()?
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
            Material::builder_compute(device)
                .pipeline_layout(pipeline_layout)
                .shader(sky_shader)
                .build()?
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
        map.insert("grid".into(), Arc::new(grid_mat));
        map.insert("default".into(), Arc::new(default_mat));
        Ok(map)
    }
    */
}

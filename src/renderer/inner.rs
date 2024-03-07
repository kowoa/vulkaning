use bevy::log;
use std::sync::{Arc, Mutex, MutexGuard};

use ash::vk;
use color_eyre::eyre::{eyre, Result};
use glam::{Mat4, Vec4};

use crate::renderer::{camera::GpuCameraData, texture::Texture};

use super::{
    buffer::AllocatedBuffer,
    camera::Camera,
    core::Core,
    descriptors::{
        DescriptorAllocator, DescriptorSetLayoutBuilder, PoolSizeRatio,
    },
    frame::Frame,
    material::Material,
    mesh::Mesh,
    model::Model,
    render_resources::RenderResources,
    shader::GraphicsShader,
    swapchain::Swapchain,
    upload_context::UploadContext,
    vkinit, vkutils,
};

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct GpuSceneData {
    pub fog_color: Vec4,     // w for exponent
    pub fog_distances: Vec4, // x for min, y for max, zw unused
    pub ambient_color: Vec4,
    pub sunlight_direction: Vec4, // w for sun power
    pub sunlight_color: Vec4,
}

#[repr(C)]
pub struct GpuObjectData {
    pub model_mat: Mat4,
}

pub const FRAME_OVERLAP: u32 = 2;
pub const MAX_OBJECTS: u32 = 10000; // Max objects per frame

pub struct RendererInner {
    pub core: Core,
    pub swapchain: Swapchain,
    pub desc_allocator: DescriptorAllocator,

    pub frame_number: u32,
    pub frames: Vec<Arc<Mutex<Frame>>>,
    pub command_pool: vk::CommandPool,

    pub scene_camera_buffer: AllocatedBuffer,
    pub upload_context: UploadContext,

    pub background_texture: Texture,
}

impl RendererInner {
    pub fn new(window: &winit::window::Window) -> Result<Self> {
        log::info!("Initializing renderer ...");

        let mut core = Core::new(window)?;
        let swapchain = Swapchain::new(&mut core, window)?;

        let mut desc_allocator = Self::create_desc_allocator(&core.device)?;
        Self::create_desc_set_layouts(&core.device, &mut desc_allocator)?;

        let upload_context = UploadContext::new(
            &core.device,
            core.queue_family_indices.get_graphics_family()?,
            core.graphics_queue,
        )?;

        let background_texture = {
            let size = window.inner_size();
            Texture::new_compute_texture(
                size.width,
                size.height,
                &core.device,
                &mut *core.get_allocator()?,
                &mut desc_allocator,
            )?
        };

        let scene_camera_buffer = {
            let scene_size = core
                .pad_uniform_buffer_size(
                    std::mem::size_of::<GpuSceneData>() as u64
                ) as u32;
            let camera_size = core
                .pad_uniform_buffer_size(
                    std::mem::size_of::<GpuCameraData>() as u64
                ) as u32;
            let size = FRAME_OVERLAP * (scene_size + camera_size);
            let offsets =
                [0, scene_size, 2 * scene_size, 2 * scene_size + camera_size];

            let mut allocator = core.get_allocator()?;
            let mut buffer = AllocatedBuffer::new(
                &core.device,
                &mut allocator,
                size as u64,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                "Scene-Camera Uniform Buffer",
                gpu_allocator::MemoryLocation::CpuToGpu,
            )?;
            buffer.set_offsets(offsets.to_vec());
            buffer
        };

        let command_pool = Self::create_command_pool(
            &core.device,
            core.queue_family_indices.get_graphics_family()?,
        )?;

        let frames = {
            let mut frames = Vec::with_capacity(FRAME_OVERLAP as usize);
            for _ in 0..FRAME_OVERLAP {
                // Call Frame constructor
                let frame = Frame::new(
                    &mut core,
                    &scene_camera_buffer,
                    &command_pool,
                    &mut desc_allocator,
                )?;

                frames.push(Arc::new(Mutex::new(frame)));
            }
            frames
        };

        Ok(Self {
            core,
            swapchain,
            frame_number: 0,
            frames,
            command_pool,
            scene_camera_buffer,
            upload_context,
            desc_allocator,
            background_texture,
        })
    }

    pub fn init_resources(
        &mut self,
        resources: &mut RenderResources,
    ) -> Result<()> {
        self.init_models(resources)?;
        self.init_textures(resources)?;
        self.init_materials(resources)?;

        Ok(())
    }

    fn get_current_frame(&self) -> Result<MutexGuard<Frame>> {
        match self.frames[(self.frame_number % FRAME_OVERLAP) as usize].lock() {
            Ok(frame) => Ok(frame),
            Err(err) => Err(eyre!(err.to_string())),
        }
    }

    fn draw_background(&mut self, cmd: vk::CommandBuffer) {
        self.background_texture.image_mut().transition_layout(
            cmd,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
            &self.core.device,
        );

        unsafe {
            self.core.device.cmd_clear_color_image(
                cmd,
                self.background_texture.image().image,
                vk::ImageLayout::GENERAL,
                &vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
                &[vk::ImageSubresourceRange {
                    aspect_mask: self.background_texture.image().aspect,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }],
            );
        }

        /*
        // Execute the compute pipeline dispatch
        // The gradient compute shader uses a 16x16 workgroup, so divide by 16
        // The compute shader will write to the draw image
        unsafe {
            self.core.device.cmd_dispatch(
                cmd,
                (self.background_texture.width() as f64 / 16.0).ceil() as u32,
                (self.background_texture.height() as f64 / 16.0).ceil() as u32,
                1,
            );
        }
        */
    }

    // MAKE SURE TO CALL THIS FUNCTION AFTER DRAWING EVERYTHING ELSE
    fn draw_grid(
        &mut self,
        cmd: vk::CommandBuffer,
        resources: &RenderResources,
        frame_index: u32,
    ) -> Result<()> {
        let device = &self.core.device;
        let grid_mat = &resources.materials["grid"];
        let grid_model = &resources.models["quad"];

        let frame = self.get_current_frame()?;

        grid_mat.bind_pipeline(cmd, device);
        grid_mat.bind_desc_sets(
            cmd,
            device,
            0,
            &[frame.scene_camera_desc_set],
            &[
                self.scene_camera_buffer.get_offset(frame_index)?,
                self.scene_camera_buffer.get_offset(frame_index + 2)?,
            ],
        );
        grid_model.draw(cmd, device)?;

        Ok(())
    }

    fn begin_renderpass(
        &self,
        cmd: vk::CommandBuffer,
        image_view: vk::ImageView,
        image_width: u32,
        image_height: u32,
    ) {
        let color_attachments = [vk::RenderingAttachmentInfo::builder()
            .image_view(image_view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            })
            .build()];
        let depth_attachment = vk::RenderingAttachmentInfo::builder()
            .image_view(self.swapchain.depth_image.view)
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            })
            .build();

        let rendering_info = vk::RenderingInfo::builder()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: image_width,
                    height: image_height,
                },
            })
            .layer_count(1)
            .color_attachments(&color_attachments)
            .depth_attachment(&depth_attachment)
            .build();

        // Begin a render pass connected to the draw image
        unsafe {
            self.core.device.cmd_begin_rendering(cmd, &rendering_info);
        }
    }

    fn end_renderpass(
        &self,
        cmd: vk::CommandBuffer,
        swapchain_image: vk::Image,
    ) {
        unsafe {
            self.core.device.cmd_end_rendering(cmd);
        }
        vkutils::transition_image_layout(
            cmd,
            swapchain_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            &self.core.device,
        );
    }

    fn begin_command_buffer(&self, cmd: vk::CommandBuffer) -> Result<()> {
        // Reset the command buffer to begin recording
        unsafe {
            self.core.device.reset_command_buffer(
                cmd,
                vk::CommandBufferResetFlags::empty(),
            )?;
        }

        // Begin command buffer recording
        let cmd_begin_info = vk::CommandBufferBeginInfo {
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
            ..Default::default()
        };
        unsafe {
            self.core
                .device
                .begin_command_buffer(cmd, &cmd_begin_info)?;
        }

        Ok(())
    }

    fn end_command_buffer(
        &self,
        cmd: vk::CommandBuffer,
        render_semaphore: vk::Semaphore,
        present_semaphore: vk::Semaphore,
        render_fence: vk::Fence,
    ) -> Result<()> {
        unsafe {
            // Finalize the main command buffer
            self.core.device.end_command_buffer(cmd)?;

            // Prepare submission to the graphics queue
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = vk::SubmitInfo {
                p_wait_dst_stage_mask: wait_stages.as_ptr(),
                wait_semaphore_count: 1,
                p_wait_semaphores: &present_semaphore,
                signal_semaphore_count: 1,
                p_signal_semaphores: &render_semaphore,
                command_buffer_count: 1,
                p_command_buffers: &cmd,
                ..Default::default()
            };
            self.core.device.queue_submit(
                self.core.graphics_queue,
                &[submit_info],
                render_fence,
            )?;
        }
        Ok(())
    }

    fn draw_geometry(
        &mut self,
        cmd: vk::CommandBuffer,
        camera: &Camera,
        resources: &RenderResources,
    ) -> Result<()> {
        let frame_index = self.frame_number % FRAME_OVERLAP;
        let scene_start_offset =
            self.scene_camera_buffer.get_offset(frame_index)?;
        let camera_start_offset =
            self.scene_camera_buffer.get_offset(frame_index + 2)?;

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
                viewproj: camera.viewproj_mat(
                    self.background_texture.width() as f32,
                    self.background_texture.height() as f32,
                ),
                near: camera.near,
                far: camera.far,
            };

            // Copy GpuCameraData struct to buffer
            self.scene_camera_buffer
                .write(&[cam_data], camera_start_offset as usize)?;
        }
        let monkey_mat = &resources.materials["default"];
        let monkey_model = &resources.models["monkey"];
        monkey_mat.bind_pipeline(cmd, &self.core.device);
        monkey_mat.bind_desc_sets(
            cmd,
            &self.core.device,
            0,
            &[self.get_current_frame()?.scene_camera_desc_set],
            &[scene_start_offset, camera_start_offset],
        );
        monkey_model.draw(cmd, &self.core.device)?;

        Ok(())
    }

    /// Returns the swapchain image index draw into
    pub fn draw_frame(
        &mut self,
        camera: &Camera,
        resources: &RenderResources,
    ) -> Result<()> {
        let (
            cmd,
            swapchain_image_index,
            render_semaphore,
            present_semaphore,
            render_fence,
        ) = {
            let frame = self.get_current_frame()?;

            // Wait until GPU has finished rendering last frame (1 sec timeout)
            unsafe {
                let fences = [frame.render_fence];
                self.core
                    .device
                    .wait_for_fences(&fences, true, 1000000000)?;
                self.core.device.reset_fences(&fences)?;
            }

            // Request image from swapchain (1 sec timeout)
            let swapchain_image_index = unsafe {
                let (index, suboptimal) =
                    self.swapchain.swapchain_loader.acquire_next_image(
                        self.swapchain.swapchain,
                        1000000000,
                        frame.present_semaphore,
                        vk::Fence::null(),
                    )?;
                if suboptimal {
                    log::warn!("Swapchain image is suboptimal");
                }
                index
            };

            (
                frame.command_buffer,
                swapchain_image_index,
                frame.render_semaphore,
                frame.present_semaphore,
                frame.render_fence,
            )
        };

        self.begin_command_buffer(cmd)?;

        self.draw_background(cmd);
        self.copy_background_texture_to_swapchain(
            cmd,
            self.swapchain.images[swapchain_image_index as usize],
        );

        self.begin_renderpass(
            cmd,
            self.swapchain.image_views[swapchain_image_index as usize],
            self.swapchain.image_extent.width,
            self.swapchain.image_extent.height,
        );
        self.set_viewport_scissor(
            cmd,
            self.swapchain.image_extent.width,
            self.swapchain.image_extent.height,
        );
        self.draw_geometry(cmd, camera, resources)?;
        self.draw_grid(cmd, resources, self.frame_number % FRAME_OVERLAP)?;
        self.end_renderpass(
            cmd,
            self.swapchain.images[swapchain_image_index as usize],
        );

        self.end_command_buffer(
            cmd,
            render_semaphore,
            present_semaphore,
            render_fence,
        )?;
        self.present_frame(swapchain_image_index, render_semaphore)?;
        self.frame_number += 1;

        Ok(())
    }

    fn present_frame(
        &mut self,
        swapchain_image_index: u32,
        render_semaphore: vk::Semaphore,
    ) -> Result<()> {
        let present_info = vk::PresentInfoKHR {
            p_swapchains: &self.swapchain.swapchain,
            swapchain_count: 1,
            p_wait_semaphores: &render_semaphore,
            wait_semaphore_count: 1,
            p_image_indices: &swapchain_image_index,
            ..Default::default()
        };
        unsafe {
            self.swapchain
                .swapchain_loader
                .queue_present(self.core.present_queue, &present_info)?;
        }
        Ok(())
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

    pub fn cleanup(mut self, resources: &mut RenderResources) {
        // Wait until all frames have finished rendering
        for frame in &self.frames {
            let frame = frame.lock().unwrap();
            unsafe {
                self.core
                    .device
                    .wait_for_fences(&[frame.render_fence], true, 1000000000)
                    .unwrap();
            }
        }

        {
            let device = &self.core.device;
            let mut allocator = self.core.get_allocator().unwrap();

            resources.cleanup(device, &mut allocator);

            self.desc_allocator.cleanup(device);
            self.upload_context.cleanup(device);

            // Destroy command pool
            unsafe {
                device.destroy_command_pool(self.command_pool, None);
            }

            // Clean up all frames
            for _ in 0..self.frames.len() {
                let frame = self.frames.pop().unwrap();
                let frame = Arc::try_unwrap(frame).unwrap();
                let frame = frame.into_inner().unwrap();
                frame.cleanup(device, &mut allocator);
            }

            // Clean up buffers
            self.scene_camera_buffer.cleanup(device, &mut allocator);
            self.background_texture.cleanup(device, &mut allocator);

            // Clean up swapchain
            self.swapchain.cleanup(device, &mut allocator);
        }

        // Clean up core Vulkan objects
        self.core.cleanup();
    }

    /// Set dynamic viewport and scissor
    fn set_viewport_scissor(
        &self,
        cmd: vk::CommandBuffer,
        width: u32,
        height: u32,
    ) {
        let viewport = vk::Viewport::builder()
            .x(0.0)
            .y(0.0)
            //.width(swapchain_image.extent.width as f32)
            //.height(swapchain_image.extent.height as f32)
            .width(width as f32)
            .height(height as f32)
            .min_depth(0.0)
            .max_depth(1.0)
            .build();
        unsafe {
            self.core.device.cmd_set_viewport(cmd, 0, &[viewport]);
        }

        let scissor = vk::Rect2D::builder()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(vk::Extent2D { width, height })
            .build();
        unsafe {
            self.core.device.cmd_set_scissor(cmd, 0, &[scissor]);
        }
    }

    /// Helper function that copies the background texture to the specified swapchain image
    fn copy_background_texture_to_swapchain(
        &mut self,
        cmd: vk::CommandBuffer,
        swapchain_image: vk::Image,
    ) {
        let device = &self.core.device;

        // Transition the draw image and swapchain image into their correct transfer layouts
        self.background_texture.image_mut().transition_layout(
            cmd,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            device,
        );
        vkutils::transition_image_layout(
            cmd,
            swapchain_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            device,
        );

        // Execute a copy from the draw image into the swapchain
        self.background_texture.image_mut().copy_to_image(
            cmd,
            swapchain_image,
            self.swapchain.image_extent,
            device,
        );

        // Transition the swapchain image to color attachment optimal layout for more drawing
        vkutils::transition_image_layout(
            cmd,
            swapchain_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            &self.core.device,
        );
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

    fn create_desc_set_layouts(
        device: &ash::Device,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<()> {
        let compute_texture_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::STORAGE_IMAGE,
                vk::ShaderStageFlags::COMPUTE,
            )
            .build(device)?;
        desc_allocator.add_layout("compute texture", compute_texture_layout);

        let graphics_texture_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::FRAGMENT,
            )
            .build(device)?;
        desc_allocator.add_layout("graphics texture", graphics_texture_layout);

        let scene_camera_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .add_binding(
                1,
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .build(device)?;
        desc_allocator.add_layout("scene-camera buffer", scene_camera_layout);

        /*
        let object_desc_set_layout = {
            // Binding 0 for GpuObjectData
            let object_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::STORAGE_BUFFER,
                vk::ShaderStageFlags::VERTEX,
                0,
            );

            let set_info = vk::DescriptorSetLayoutCreateInfo {
                binding_count: 1,
                p_bindings: &object_bind,
                ..Default::default()
            };
            unsafe { device.create_descriptor_set_layout(&set_info, None)? }
        };

        desc_allocator.add_layout("object", object_desc_set_layout);
        */

        Ok(())
    }

    fn create_desc_allocator(
        device: &ash::Device,
    ) -> Result<DescriptorAllocator> {
        let ratios = [
            PoolSizeRatio {
                // For the camera buffer
                desc_type: vk::DescriptorType::UNIFORM_BUFFER,
                ratio: 1.0,
            },
            PoolSizeRatio {
                // For the scene params buffer
                desc_type: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                ratio: 1.0,
            },
            PoolSizeRatio {
                // For the object buffer
                desc_type: vk::DescriptorType::STORAGE_BUFFER,
                ratio: 1.0,
            },
            PoolSizeRatio {
                desc_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                // For textures
                ratio: 1.0,
            },
        ];

        let global_desc_allocator =
            DescriptorAllocator::new(device, 10, &ratios)?;

        Ok(global_desc_allocator)
    }

    /// Upload all models to the GPU
    fn init_models(&mut self, resources: &mut RenderResources) -> Result<()> {
        let mut allocator = self.core.get_allocator()?;

        // Upload asset models to the GPU
        for (_, model) in resources.models.iter_mut() {
            model.upload(
                &self.core.device,
                &mut allocator,
                &self.upload_context,
            )?;
        }
        // Upload other models to the GPU
        let quad = Mesh::new_quad();
        let mut quad = Model::new(vec![quad]);
        quad.upload(&self.core.device, &mut allocator, &self.upload_context)?;
        resources.models.insert("quad".into(), quad);

        Ok(())
    }

    /// Upload all textures to the GPU
    fn init_textures(&mut self, resources: &mut RenderResources) -> Result<()> {
        for (_, texture) in resources.textures.iter_mut() {
            texture.init_graphics_texture(
                &self.core.device,
                &mut *self.core.get_allocator()?,
                &mut self.desc_allocator,
                &self.upload_context,
            )?;
        }
        Ok(())
    }

    /// Create materials and insert them into RenderResources
    fn init_materials(&self, resources: &mut RenderResources) -> Result<()> {
        let scene_camera_layout =
            self.desc_allocator.get_layout("scene-camera buffer")?;
        /*
        let graphics_texture_layout =
            self.desc_allocator.get_layout("graphics texture")?;
        let compute_texture_layout =
            self.desc_allocator.get_layout("compute texture")?;
        */

        let default_mat = {
            let set_layouts = [*scene_camera_layout];
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&set_layouts)
                .build();
            let pipeline_layout = unsafe {
                self.core
                    .device
                    .create_pipeline_layout(&pipeline_layout_info, None)?
            };
            Material::builder_graphics(&self.core.device)
                .pipeline_layout(pipeline_layout)
                .shader(GraphicsShader::new("default", &self.core.device)?)
                .color_attachment_format(self.swapchain.image_format)
                .depth_attachment_format(self.swapchain.depth_image.format)
                .build()?
        };
        resources.materials.insert("default".into(), default_mat);

        let grid_mat = {
            let set_layouts = [*scene_camera_layout];
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&set_layouts)
                .build();
            let pipeline_layout = unsafe {
                self.core
                    .device
                    .create_pipeline_layout(&pipeline_layout_info, None)?
            };
            Material::builder_graphics(&self.core.device)
                .pipeline_layout(pipeline_layout)
                .shader(GraphicsShader::new("grid", &self.core.device)?)
                .color_attachment_format(self.swapchain.image_format)
                .depth_attachment_format(self.swapchain.depth_image.format)
                .build()?
        };
        resources.materials.insert("grid".into(), grid_mat);

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

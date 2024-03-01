use std::{
    ffi::CString,
    sync::{Arc, Mutex, MutexGuard},
};

use ash::vk;
use color_eyre::eyre::{eyre, Result};
use egui_ash::EguiCommand;
use glam::{Mat4, Vec3, Vec4};
use gpu_allocator::vulkan::Allocator;

use crate::renderer::{camera::GpuCameraData, resources::scene::GpuSceneData};

use super::{
    buffer::AllocatedBuffer,
    camera::Camera,
    core::Core,
    descriptors::{
        DescriptorAllocator, DescriptorSetLayoutBuilder, PoolSizeRatio,
    },
    frame::Frame,
    image::{AllocatedImage, AllocatedImageCreateInfo},
    resources::Resources,
    swapchain::Swapchain,
    upload_context::UploadContext,
    vkinit, vkutils,
};

pub const FRAME_OVERLAP: u32 = 2;
pub const MAX_OBJECTS: u32 = 10000; // Max objects per frame

pub struct RendererInner {
    pub core: Core,
    pub swapchain: Swapchain,
    pub resources: Resources,
    pub desc_allocator: DescriptorAllocator,

    pub frame_number: u32,
    pub frames: Vec<Arc<Mutex<Frame>>>,
    pub command_pool: vk::CommandPool,

    pub draw_image: AllocatedImage, // Image to render into
    pub scene_camera_buffer: AllocatedBuffer,
    pub upload_context: UploadContext,

    pub first_draw: bool,
}

impl RendererInner {
    pub fn new(
        window: &winit::window::Window,
        window_req_instance_exts: Vec<CString>,
        window_req_device_exts: Vec<CString>,
    ) -> Result<Self> {
        log::info!("Initializing renderer ...");

        let mut core = Core::new(
            window,
            window_req_instance_exts,
            window_req_device_exts,
        )?;
        let swapchain = Swapchain::new(&mut core, window)?;

        let mut desc_allocator = Self::create_desc_allocator(&core.device)?;
        Self::create_desc_set_layouts(&core.device, &mut desc_allocator)?;

        let upload_context = UploadContext::new(
            &core.device,
            core.queue_family_indices.get_graphics_family()?,
            core.graphics_queue,
        )?;

        let draw_image = {
            let size = window.inner_size();
            Self::create_draw_image(
                size.width,
                size.height,
                &core.device,
                &mut *core.get_allocator()?,
                &mut desc_allocator,
            )?
        };

        let resources = Resources::new(
            &mut core,
            &swapchain,
            &upload_context,
            &mut desc_allocator,
            &draw_image,
        )?;

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
            resources,
            frame_number: 0,
            frames,
            command_pool,
            scene_camera_buffer,
            upload_context,
            first_draw: true,
            draw_image,
            desc_allocator,
        })
    }

    fn get_current_frame(&self) -> Result<MutexGuard<Frame>> {
        match self.frames[(self.frame_number % FRAME_OVERLAP) as usize].lock() {
            Ok(frame) => Ok(frame),
            Err(err) => Err(eyre!(err.to_string())),
        }
    }

    fn draw_background(&mut self, cmd: vk::CommandBuffer) {
        self.draw_image.transition_layout(
            cmd,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
            &self.core.device,
        );

        let effect = &self.resources.background_effects
            [self.resources.current_background_effects_index];
        let material = &effect.material;
        let device = &self.core.device;

        // Bind the gradient drawing compute pipeline
        material.bind_pipeline(cmd, device);

        // Bind the descriptor set containing the draw image to the compute pipeline
        material.bind_desc_sets(
            cmd,
            device,
            0,
            &[self.draw_image.desc_set.unwrap()],
            &[],
        );

        // Update push constants
        material.update_push_constants(
            cmd,
            device,
            vk::ShaderStageFlags::COMPUTE,
            bytemuck::cast_slice(&[effect.data]),
        );

        // Execute the compute pipeline dispatch
        // The gradient compute shader uses a 16x16 workgroup, so divide by 16
        // The compute shader will write to the draw image
        unsafe {
            self.core.device.cmd_dispatch(
                cmd,
                (self.draw_image.extent.width as f64 / 16.0).ceil() as u32,
                (self.draw_image.extent.height as f64 / 16.0).ceil() as u32,
                1,
            );
        }
    }

    // MAKE SURE TO CALL THIS FUNCTION AFTER DRAWING EVERY OBJECT
    fn draw_grid(
        &mut self,
        cmd: vk::CommandBuffer,
        frame_index: u32,
    ) -> Result<()> {
        let device = &self.core.device;
        let grid_mat = self.resources.materials["grid"].as_ref();
        let grid_model = self.resources.models["quad"].as_ref();
        grid_mat.bind_pipeline(cmd, device);

        let frame = self.get_current_frame()?;
        let scene_start_offset =
            self.scene_camera_buffer.offsets.as_ref().unwrap()
                [frame_index as usize];
        let camera_start_offset =
            self.scene_camera_buffer.offsets.as_ref().unwrap()
                [frame_index as usize + 2];
        grid_mat.bind_desc_sets(
            cmd,
            device,
            0,
            &[frame.global_desc_set],
            &[scene_start_offset, camera_start_offset],
        );
        grid_mat.update_push_constants(
            cmd,
            device,
            vk::ShaderStageFlags::VERTEX,
            bytemuck::cast_slice(&[Mat4::IDENTITY]),
        );
        grid_model.draw(cmd, device)?;

        Ok(())
    }

    fn draw_geometry(&mut self, cmd: vk::CommandBuffer) -> Result<()> {
        self.draw_image.transition_layout(
            cmd,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            &self.core.device,
        );
        let color_attachments = [vk::RenderingAttachmentInfo::builder()
            .image_view(self.draw_image.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            })
            .build()];

        let depth_clear = vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        };
        let depth_attachment = vk::RenderingAttachmentInfo::builder()
            .image_view(self.swapchain.depth_image.view)
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(depth_clear)
            .build();

        let rendering_info = vk::RenderingInfo::builder()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: self.draw_image.extent.width,
                    height: self.draw_image.extent.height,
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

        // RENDERING COMMANDS START

        self.draw_render_objects(
            self.draw_image.extent.width,
            self.draw_image.extent.height,
            0,
            self.resources.render_objs.len(),
        )?;
        self.draw_grid(cmd, self.frame_number % FRAME_OVERLAP)?;

        // RENDERING COMMANDS END

        // End the renderpass
        unsafe {
            self.core.device.cmd_end_rendering(cmd);
        }

        Ok(())
    }

    /// Returns the swapchain image index draw into
    pub fn draw_frame(
        &mut self,
        width: u32,
        height: u32,
        mut egui_cmd: Option<EguiCommand>,
    ) -> Result<u32> {
        if self.first_draw {
            self.first_draw = false;
            // Update swapchain if it needs to be recreated
            let egui_cmd_take = egui_cmd.take();
            if let Some(mut cmd) = egui_cmd_take {
                cmd.update_swapchain(egui_ash::SwapchainUpdateInfo {
                    width,
                    height,
                    swapchain_images: self.swapchain.images.clone(),
                    surface_format: self.swapchain.image_format,
                });
                egui_cmd = Some(cmd);
            }
        }

        let (
            swapchain_image_index,
            cmd,
            render_semaphore,
            present_semaphore,
            render_fence,
        ) = {
            let frame = self.get_current_frame()?;
            let fences = [frame.render_fence];

            // Wait until GPU has finished rendering last frame (1 sec timeout)
            unsafe {
                self.core
                    .device
                    .wait_for_fences(&fences, true, 1000000000)?;
                self.core.device.reset_fences(&fences)?;
            }

            // Request image from swapchain (1 sec timeout)
            let (swapchain_image_index, _) = unsafe {
                self.swapchain.swapchain_loader.acquire_next_image(
                    self.swapchain.swapchain,
                    1000000000,
                    frame.present_semaphore,
                    vk::Fence::null(),
                )?
            };

            (
                swapchain_image_index,
                frame.command_buffer,
                frame.render_semaphore,
                frame.present_semaphore,
                frame.render_fence,
            )
        };

        // Begin command buffer recording
        {
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
        }

        // Set dynamic viewport and scissor
        {
            let viewport = vk::Viewport::builder()
                .x(0.0)
                .y(0.0)
                .width(self.draw_image.extent.width as f32)
                .height(self.draw_image.extent.height as f32)
                .min_depth(0.0)
                .max_depth(1.0)
                .build();
            unsafe {
                self.core.device.cmd_set_viewport(cmd, 0, &[viewport]);
            }

            let scissor = vk::Rect2D::builder()
                .offset(vk::Offset2D { x: 0, y: 0 })
                .extent(vk::Extent2D {
                    width: self.draw_image.extent.width,
                    height: self.draw_image.extent.height,
                })
                .build();
            unsafe {
                self.core.device.cmd_set_scissor(cmd, 0, &[scissor]);
            }
        }

        self.draw_geometry(cmd)?;

        // Copy draw image to swapchain image
        {
            let device = &self.core.device;
            let swapchain_image =
                self.swapchain.images[swapchain_image_index as usize];

            // Transition the draw image and swapchain image into their correct transfer layouts
            self.draw_image.transition_layout(
                cmd,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
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
            self.draw_image.copy_to_image(
                cmd,
                swapchain_image,
                self.swapchain.image_extent,
                device,
            );

            // Transition swapchain image to COLOR_ATTACHMENT_OPTIMAL for use with egui
            vkutils::transition_image_layout(
                cmd,
                swapchain_image,
                vk::ImageAspectFlags::COLOR,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                device,
            );
        }

        // Record egui commands
        if let Some(egui_cmd) = egui_cmd {
            egui_cmd.record(cmd, swapchain_image_index as usize);
        }

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

        Ok(swapchain_image_index)
    }

    pub fn present_frame(&mut self, swapchain_image_index: u32) -> Result<()> {
        {
            let frame = self.get_current_frame()?;
            let present_info = vk::PresentInfoKHR {
                p_swapchains: &self.swapchain.swapchain,
                swapchain_count: 1,
                p_wait_semaphores: &frame.render_semaphore,
                wait_semaphore_count: 1,
                p_image_indices: &swapchain_image_index,
                ..Default::default()
            };

            unsafe {
                self.swapchain
                    .swapchain_loader
                    .queue_present(self.core.present_queue, &present_info)?;
            }
        }

        self.frame_number += 1;

        Ok(())
    }

    pub fn draw_render_objects(
        &mut self,
        width: u32,
        height: u32,
        first_index: usize,
        count: usize,
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
            let mut cam = Camera::default();
            cam.set_position(Vec3::new(0.0, 4.0, 10.0));
            cam.look_at(Vec3::ZERO);

            // Fill a GpuCameraData struct
            let view = cam.view_mat();
            let proj = cam.proj_mat(width as f32, height as f32);
            let cam_data = GpuCameraData {
                view,
                proj,
                viewproj: proj * view,
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
            let render_obj = &self.resources.render_objs[instance_index];
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

    pub fn cleanup(mut self) {
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

            self.desc_allocator.cleanup(device);
            self.upload_context.cleanup(device);
            self.resources.cleanup(device, &mut allocator);

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
            self.draw_image.cleanup(device, &mut allocator);

            // Clean up swapchain
            self.swapchain.cleanup(device, &mut allocator);
        }

        // Clean up core Vulkan objects
        self.core.cleanup();
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
        let global_desc_set_layout = {
            // Binding 0 for GpuSceneData
            let scene_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
            );
            // Binding 1 for GpuCameraData
            let camera_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::ShaderStageFlags::VERTEX,
                1,
            );
            let bindings = [scene_bind, camera_bind];

            let set_info = vk::DescriptorSetLayoutCreateInfo {
                binding_count: bindings.len() as u32,
                flags: vk::DescriptorSetLayoutCreateFlags::empty(),
                p_bindings: bindings.as_ptr(),
                ..Default::default()
            };
            unsafe { device.create_descriptor_set_layout(&set_info, None)? }
        };

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

        let single_texture_desc_set_layout = {
            let texture_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::FRAGMENT,
                0,
            );

            let set_info = vk::DescriptorSetLayoutCreateInfo {
                binding_count: 1,
                p_bindings: &texture_bind,
                ..Default::default()
            };
            unsafe { device.create_descriptor_set_layout(&set_info, None)? }
        };

        desc_allocator.add_layout("global", global_desc_set_layout);
        desc_allocator.add_layout("object", object_desc_set_layout);
        desc_allocator
            .add_layout("single texture", single_texture_desc_set_layout);

        Ok(())
    }

    /// Helper function that creates the descriptor pool and descriptor sets
    fn create_draw_image(
        width: u32,
        height: u32,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<AllocatedImage> {
        let draw_image_desc_set = {
            let layout = DescriptorSetLayoutBuilder::new()
                .add_binding(
                    0,
                    vk::DescriptorType::STORAGE_IMAGE,
                    vk::ShaderStageFlags::COMPUTE,
                )
                .build(device)?;
            desc_allocator.add_layout("draw image", layout);
            desc_allocator.allocate(device, "draw image")?
        };

        let draw_image = {
            let extent = vk::Extent3D {
                width,
                height,
                depth: 1,
            };
            let usage_flags = vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::COLOR_ATTACHMENT;
            let create_info = AllocatedImageCreateInfo {
                format: vk::Format::R16G16B16A16_SFLOAT,
                extent,
                usage_flags,
                aspect_flags: vk::ImageAspectFlags::COLOR,
                name: "Draw image".into(),
                desc_set: Some(draw_image_desc_set),
            };
            AllocatedImage::new(&create_info, device, allocator)?
        };

        let draw_image_info = [vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::GENERAL)
            .image_view(draw_image.view)
            .build()];
        let draw_image_write = vk::WriteDescriptorSet::builder()
            .dst_binding(0)
            .dst_set(draw_image_desc_set)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(&draw_image_info)
            .build();
        unsafe {
            device.update_descriptor_sets(&[draw_image_write], &[]);
        }

        Ok(draw_image)
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
}

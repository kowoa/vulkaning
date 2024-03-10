use ash::vk;
use bevy::log;
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;

use crate::renderer::{buffer::AllocatedBuffer, core::Core};

use super::{
    descriptors::{DescriptorAllocator, DescriptorWriter},
    gpu_data::{GpuCameraData, GpuSceneData},
    inner::DrawContext,
    texture::Texture,
    vkutils,
};

#[derive(Debug)]
pub struct Frame {
    present_semaphore: vk::Semaphore, // Signals when the swapchain is ready to present
    render_semaphore: vk::Semaphore,  // Signals when rendering is done
    render_fence: vk::Fence, // Signals when rendering commands all get executed
    command_buffer: vk::CommandBuffer,
    desc_allocator: DescriptorAllocator,

    scene_buffer: AllocatedBuffer,
}

impl Frame {
    pub fn new(
        core: &mut Core,
        allocator: &mut Allocator,
        command_pool: &vk::CommandPool,
    ) -> Result<Self> {
        let device = &core.device;

        // Create command buffer
        let command_buffer = Self::create_command_buffer(device, command_pool)?;

        // Create semaphores and fences
        let (present_semaphore, render_semaphore, render_fence) =
            Self::create_sync_objs(device)?;

        // Create descriptor allocator exclusive to this frame
        let desc_allocator = DescriptorAllocator::new(&core.device, 1000)?;

        // Allocate a new uniform buffer for the scene data
        let scene_buffer = AllocatedBuffer::new(
            &core.device,
            allocator,
            std::mem::size_of::<GpuSceneData>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            "Scene Buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        Ok(Self {
            present_semaphore,
            render_semaphore,
            render_fence,
            command_buffer,
            desc_allocator,

            scene_buffer,
        })
    }

    pub fn draw(&mut self, mut ctx: DrawContext) -> Result<()> {
        // Wait until GPU has finished rendering last frame (1 sec timeout)
        unsafe {
            let fences = [self.render_fence];
            ctx.device.wait_for_fences(&fences, true, 1000000000)?;
            ctx.device.reset_fences(&fences)?;
        }

        self.desc_allocator.clear_pools(&ctx.device)?;

        // Create a descriptor set for the scene buffer
        let scene_desc_set = self.desc_allocator.allocate(
            &ctx.device,
            ctx.resources.lock().unwrap().desc_set_layouts["scene buffer"],
        )?;

        // Write to the buffer
        let scene_data = GpuSceneData {
            cam_data: GpuCameraData {
                viewproj: ctx.camera.viewproj_mat(
                    ctx.swapchain.image_extent.width as f32,
                    ctx.swapchain.image_extent.height as f32,
                ),
                near: ctx.camera.near,
                far: ctx.camera.far,
            },
            ..Default::default()
        };
        self.scene_buffer.write(&[scene_data], 0)?;

        // Update the scene descriptor set with the updated scene buffer
        let mut writer = DescriptorWriter::new();
        writer.write_buffer(
            0,
            self.scene_buffer.buffer,
            self.scene_buffer.size,
            0,
            vk::DescriptorType::UNIFORM_BUFFER,
        );
        writer.update_set(&ctx.device, scene_desc_set);

        // Request image from swapchain (1 sec timeout)
        let swapchain_image_index = unsafe {
            let (index, suboptimal) =
                ctx.swapchain.swapchain_loader.acquire_next_image(
                    ctx.swapchain.swapchain,
                    1000000000,
                    self.present_semaphore,
                    vk::Fence::null(),
                )?;
            if suboptimal {
                log::warn!("Swapchain image is suboptimal");
            }
            index
        };

        //----------------------------------------------------------------------
        let cmd = self.command_buffer;
        self.begin_command_buffer(cmd, &ctx)?;
        //----------------------------------------------------------------------

        // Compute operations
        self.draw_background(
            cmd,
            &ctx,
            &mut ctx.background_texture.lock().unwrap(),
        )?;
        self.copy_background_texture_to_swapchain(
            cmd,
            &ctx.device,
            &mut ctx.background_texture.lock().unwrap(),
            ctx.swapchain.images[swapchain_image_index as usize],
            ctx.swapchain.image_extent,
        );

        // Render operations
        self.begin_renderpass(swapchain_image_index, cmd, &ctx);
        self.set_viewport_scissor(
            cmd,
            &ctx.device,
            ctx.swapchain.image_extent.width,
            ctx.swapchain.image_extent.height,
        );
        self.draw_geometry(cmd, &mut ctx, scene_desc_set)?;
        self.draw_grid(cmd, &ctx, scene_desc_set)?;
        self.end_renderpass(swapchain_image_index, cmd, &ctx);

        //----------------------------------------------------------------------
        self.end_command_buffer(cmd, &ctx)?;
        self.present(swapchain_image_index, &ctx)?;
        //----------------------------------------------------------------------

        Ok(())
    }

    fn present(
        &self,
        swapchain_image_index: u32,
        ctx: &DrawContext,
    ) -> Result<()> {
        let present_info = vk::PresentInfoKHR {
            p_swapchains: &ctx.swapchain.swapchain,
            swapchain_count: 1,
            p_wait_semaphores: &self.render_semaphore, // Wait until rendering is done before presenting
            wait_semaphore_count: 1,
            p_image_indices: &swapchain_image_index,
            ..Default::default()
        };
        unsafe {
            ctx.swapchain
                .swapchain_loader
                .queue_present(ctx.present_queue, &present_info)?;
        }
        Ok(())
    }

    /// Call this function AFTER starting a renderpass
    fn draw_background(
        &mut self,
        cmd: vk::CommandBuffer,
        ctx: &DrawContext,
        background_texture: &mut Texture,
    ) -> Result<()> {
        background_texture.image_mut().transition_layout(
            cmd,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
            &ctx.device,
        );

        unsafe {
            ctx.device.cmd_clear_color_image(
                cmd,
                background_texture.image().image,
                vk::ImageLayout::GENERAL,
                &vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
                &[vk::ImageSubresourceRange {
                    aspect_mask: background_texture.image().aspect,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }],
            );
        }

        Ok(())

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

    /// Call this function AFTER starting a renderpass
    pub fn draw_geometry(
        &mut self,
        cmd: vk::CommandBuffer,
        ctx: &mut DrawContext,
        scene_desc_set: vk::DescriptorSet,
    ) -> Result<()> {
        let resources = ctx.resources.lock().unwrap();
        let graphics_texture_desc_set = self.desc_allocator.allocate(
            &ctx.device,
            resources.desc_set_layouts["graphics texture"],
        )?;

        // Update the texture descriptor set
        let mut writer = DescriptorWriter::new();
        writer.write_image(
            0,
            resources.textures["backpack"].image().view,
            resources.textures["backpack"].sampler().unwrap(),
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        );
        writer.update_set(&ctx.device, graphics_texture_desc_set);

        let monkey_mat = &resources.materials["textured"];
        let monkey_model = &resources.models["backpack"];
        monkey_mat.bind_pipeline(cmd, &ctx.device);
        monkey_mat.bind_desc_sets(
            cmd,
            &ctx.device,
            0,
            &[scene_desc_set, graphics_texture_desc_set],
            &[],
        );
        monkey_model.draw(cmd, &ctx.device)?;
        self.draw_grid(cmd, ctx, scene_desc_set)?;

        Ok(())
    }

    // MAKE SURE TO CALL THIS FUNCTION AFTER DRAWING EVERYTHING ELSE
    fn draw_grid(
        &mut self,
        cmd: vk::CommandBuffer,
        ctx: &DrawContext,
        scene_desc_set: vk::DescriptorSet,
    ) -> Result<()> {
        let resources = ctx.resources.lock().unwrap();
        let grid_mat = &resources.materials["grid"];
        let grid_model = &resources.models["quad"];

        grid_mat.bind_pipeline(cmd, &ctx.device);
        grid_mat.bind_desc_sets(cmd, &ctx.device, 0, &[scene_desc_set], &[]);
        grid_model.draw(cmd, &ctx.device)?;

        Ok(())
    }

    fn begin_command_buffer(
        &self,
        cmd: vk::CommandBuffer,
        ctx: &DrawContext,
    ) -> Result<()> {
        // Reset the command buffer to begin recording
        unsafe {
            ctx.device.reset_command_buffer(
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
            ctx.device.begin_command_buffer(cmd, &cmd_begin_info)?;
        }

        Ok(())
    }

    fn end_command_buffer(
        &self,
        cmd: vk::CommandBuffer,
        ctx: &DrawContext,
    ) -> Result<()> {
        unsafe {
            // Finalize the main command buffer
            ctx.device.end_command_buffer(cmd)?;

            // Prepare submission to the graphics queue
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = vk::SubmitInfo {
                p_wait_dst_stage_mask: wait_stages.as_ptr(),
                wait_semaphore_count: 1,
                p_wait_semaphores: &self.present_semaphore, // Wait for presentation to finish
                signal_semaphore_count: 1,
                p_signal_semaphores: &self.render_semaphore, // Signal rendering is done
                command_buffer_count: 1,
                p_command_buffers: &cmd,
                ..Default::default()
            };
            ctx.device.queue_submit(
                ctx.graphics_queue,
                &[submit_info],
                self.render_fence, // Signal when the command buffer finishes executing
            )?;
        }
        Ok(())
    }

    fn begin_renderpass(
        &self,
        swapchain_image_index: u32,
        cmd: vk::CommandBuffer,
        ctx: &DrawContext,
    ) {
        let color_attachments = [vk::RenderingAttachmentInfo::builder()
            .image_view(
                ctx.swapchain.image_views[swapchain_image_index as usize],
            )
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
            .image_view(ctx.swapchain.depth_image.view)
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
                    width: ctx.swapchain.image_extent.width,
                    height: ctx.swapchain.image_extent.height,
                },
            })
            .layer_count(1)
            .color_attachments(&color_attachments)
            .depth_attachment(&depth_attachment)
            .build();

        // Begin a render pass connected to the draw image
        unsafe {
            ctx.device.cmd_begin_rendering(cmd, &rendering_info);
        }
    }

    fn end_renderpass(
        &self,
        swapchain_image_index: u32,
        cmd: vk::CommandBuffer,
        ctx: &DrawContext,
    ) {
        unsafe {
            ctx.device.cmd_end_rendering(cmd);
        }
        vkutils::transition_image_layout(
            cmd,
            ctx.swapchain.images[swapchain_image_index as usize],
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            &ctx.device,
        );
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            self.scene_buffer.cleanup(device, allocator);
            device.destroy_semaphore(self.render_semaphore, None);
            device.destroy_semaphore(self.present_semaphore, None);
            device.destroy_fence(self.render_fence, None);
            self.desc_allocator.cleanup(device);
        }
    }

    pub fn render_fence(&self) -> vk::Fence {
        self.render_fence
    }

    /// Helper function that copies the background texture to the specified swapchain image
    fn copy_background_texture_to_swapchain(
        &mut self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
        background_texture: &mut Texture,
        swapchain_image: vk::Image,
        swapchain_extent: vk::Extent2D,
    ) {
        // Transition the draw image and swapchain image into their correct transfer layouts
        background_texture.image_mut().transition_layout(
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
        background_texture.image_mut().copy_to_image(
            cmd,
            swapchain_image,
            swapchain_extent,
            device,
        );

        // Transition the swapchain image to color attachment optimal layout for more drawing
        vkutils::transition_image_layout(
            cmd,
            swapchain_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            device,
        );
    }

    /// Set dynamic viewport and scissor
    fn set_viewport_scissor(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
        width: u32,
        height: u32,
    ) {
        let viewport = vk::Viewport::builder()
            .x(0.0)
            .y(0.0)
            .width(width as f32)
            .height(height as f32)
            .min_depth(0.0)
            .max_depth(1.0)
            .build();
        unsafe {
            device.cmd_set_viewport(cmd, 0, &[viewport]);
        }

        let scissor = vk::Rect2D::builder()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(vk::Extent2D { width, height })
            .build();
        unsafe {
            device.cmd_set_scissor(cmd, 0, &[scissor]);
        }
    }

    fn create_command_buffer(
        device: &ash::Device,
        command_pool: &vk::CommandPool,
    ) -> Result<vk::CommandBuffer> {
        let buffer_info = vk::CommandBufferAllocateInfo {
            command_pool: *command_pool,
            command_buffer_count: 1,
            level: vk::CommandBufferLevel::PRIMARY,
            ..Default::default()
        };
        let command_buffer =
            unsafe { device.allocate_command_buffers(&buffer_info)?[0] };

        Ok(command_buffer)
    }

    fn create_sync_objs(
        device: &ash::Device,
    ) -> Result<(vk::Semaphore, vk::Semaphore, vk::Fence)> {
        let fence_info = vk::FenceCreateInfo {
            // Fence starts out signaled so we can wait on it for the first frame
            flags: vk::FenceCreateFlags::SIGNALED,
            ..Default::default()
        };
        let render_fence = unsafe { device.create_fence(&fence_info, None)? };

        let sem_info = vk::SemaphoreCreateInfo::default();
        let present_semaphore =
            unsafe { device.create_semaphore(&sem_info, None)? };
        let render_semaphore =
            unsafe { device.create_semaphore(&sem_info, None)? };

        Ok((present_semaphore, render_semaphore, render_fence))
    }
}

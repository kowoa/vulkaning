use ash::vk;
use bevy::log;
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;

use crate::renderer::{buffer::AllocatedBuffer, core::Core};

use super::{
    descriptors::{DescriptorAllocator, DescriptorWriter}, gpu_data::{GpuCameraData, GpuSceneData}, inner::DrawContext, swapchain::Swapchain, texture::Texture, vkutils
};

#[derive(Debug)]
pub struct Frame {
    present_semaphore: vk::Semaphore, // Signals when the swapchain is ready to present
    render_semaphore: vk::Semaphore,  // Signals when rendering is done
    pub render_fence: vk::Fence, // Signals when rendering commands all get executed
    command_buffer: vk::CommandBuffer,
    desc_allocator: DescriptorAllocator,

    background_texture: Texture,
}

impl Frame {
    pub fn new(
        core: &mut Core,
        swapchain: &Swapchain,
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

        // Create background compute texture exclusive to this frame
        let background_texture = {
            let size = swapchain.image_extent;
            Texture::new_compute_texture(
                size.width,
                size.height,
                &core.device,
                allocator,
            )?
        };


        Ok(Self {
            present_semaphore,
            render_semaphore,
            render_fence,
            command_buffer,
            desc_allocator,
            background_texture,
        })
    }

    pub fn draw(&mut self, ctx: DrawContext) -> Result<()> {
        // Wait until GPU has finished rendering last frame (1 sec timeout)
        unsafe {
            let fences = [self.render_fence];
            ctx
                .device
                .wait_for_fences(&fences, true, 1000000000)?;
            ctx.device.reset_fences(&fences)?;
        }

        self.desc_allocator.clear_pools(&ctx.device)?;

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

        let cmd = self.command_buffer;

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

    /// Call this function AFTER starting a renderpass
    fn draw_background(&mut self, ctx: &mut DrawContext) {
        self.background_texture.image_mut().transition_layout(
            ctx.cmd,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
            ctx.device,
        );

        unsafe {
            ctx.device.cmd_clear_color_image(
                ctx.cmd,
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


    /// Call this function AFTER starting a renderpass
    pub fn draw_geometry(&mut self, ctx: &mut DrawContext) -> Result<()> {
        // Allocate a new uniform buffer for the scene data
        let mut scene_buffer = AllocatedBuffer::new(
            ctx.device,
            ctx.allocator,
            std::mem::size_of::<GpuSceneData>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            "Scene Buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
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
        scene_buffer.write(&[scene_data], 0)?;

        // Create a descriptor set for the scene data
        let scene_desc_set = ctx
            .desc_allocator
            .allocate(ctx.device, *ctx.desc_set_layouts.get("scene buffer")?)?;

        // Update the descriptor set with the new scene buffer
        let mut writer = DescriptorWriter::new();
        writer.write_buffer(
            0,
            scene_buffer.buffer,
            scene_buffer.size,
            0,
            vk::DescriptorType::UNIFORM_BUFFER,
        );
        writer.update_set(ctx.device, scene_desc_set);

        let monkey_mat = &resources.materials["textured"];
        let monkey_model = &resources.models["backpack"];
        monkey_mat.bind_pipeline(cmd, &self.core.device);
        monkey_mat.bind_desc_sets(
            cmd,
            &self.core.device,
            0,
            &[
                self.get_current_frame().scene_desc
                resources.textures["backpack"].desc_set(),
            ],
            &[scene_start_offset, camera_start_offset],
        );
        monkey_model.draw(cmd, &self.core.device)?;
        self.draw_grid(ctx, scene_desc_set)?;

        // Destroy the scene buffer
        scene_buffer.cleanup(ctx.device, ctx.allocator);

        Ok(())
    }

    // MAKE SURE TO CALL THIS FUNCTION AFTER DRAWING EVERYTHING ELSE
    fn draw_grid(
        &mut self,
        ctx: &mut DrawContext,
        scene_desc_set: vk::DescriptorSet,
    ) -> Result<()> {
        let grid_mat = &ctx.resources.materials["grid"];
        let grid_model = &ctx.resources.models["quad"];

        grid_mat.bind_pipeline(ctx.cmd, ctx.device);
        grid_mat.bind_desc_sets(ctx.cmd, ctx.device, 0, &[scene_desc_set], &[]);
        grid_model.draw(ctx.cmd, ctx.device)?;

        Ok(())
    }

    /// Helper function that copies the background texture to the specified swapchain image
    fn copy_background_texture_to_swapchain(
        &mut self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
        swapchain_image: vk::Image,
        swapchain_extent: vk::Extent2D,
    ) {
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


    pub fn cleanup(self, device: &ash::Device) {
        unsafe {
            device.destroy_semaphore(self.render_semaphore, None);
            device.destroy_semaphore(self.present_semaphore, None);
            device.destroy_fence(self.render_fence, None);
            self.desc_allocator.cleanup(device);
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

use ash::vk;
use color_eyre::eyre::Result;

use crate::renderer::{
    buffer::AllocatedBuffer, core::Core, descriptors::DescriptorAllocator,
};

use super::{
    descriptors::DescriptorWriter,
    gpu_data::{GpuCameraData, GpuSceneData},
    DrawContext,
};

#[derive(Debug)]
pub struct Frame {
    pub present_semaphore: vk::Semaphore, // Signals when the swapchain is ready to present
    pub render_semaphore: vk::Semaphore,  // Signals when rendering is done
    pub render_fence: vk::Fence, // Signals when rendering commands all get executed
    pub command_buffer: vk::CommandBuffer,
    pub desc_allocator: DescriptorAllocator,
}

impl Frame {
    pub fn new(
        core: &mut Core,
        command_pool: &vk::CommandPool,
    ) -> Result<Self> {
        let device = &core.device;
        let desc_allocator = DescriptorAllocator::new(device, 1000)?;

        // Create command buffer
        let command_buffer = Self::create_command_buffer(device, command_pool)?;

        // Create semaphores and fences
        let (present_semaphore, render_semaphore, render_fence) =
            Self::create_sync_objs(device)?;

        Ok(Self {
            present_semaphore,
            render_semaphore,
            render_fence,
            command_buffer,
            desc_allocator,
        })
    }

    pub fn draw_geometry(&mut self, ctx: DrawContext) -> Result<()> {
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
        let scene_desc_set = self
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

        // Destroy the scene buffer
        scene_buffer.cleanup(ctx.device, ctx.allocator);

        Ok(())
    }

    pub fn cleanup(self, device: &ash::Device) {
        unsafe {
            self.desc_allocator.cleanup(device);
            device.destroy_semaphore(self.render_semaphore, None);
            device.destroy_semaphore(self.present_semaphore, None);
            device.destroy_fence(self.render_fence, None);
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

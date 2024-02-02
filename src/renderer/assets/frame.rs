use ash::vk;
use glam::Mat4;
use gpu_allocator::vulkan::Allocator;

use crate::renderer::memory::AllocatedBuffer;

struct CameraData {
    view: Mat4,
    proj: Mat4,
    viewproj: Mat4,
}

pub struct Frame {
    pub present_semaphore: vk::Semaphore,
    pub render_semaphore: vk::Semaphore,
    pub render_fence: vk::Fence,
    pub command_pool: vk::CommandPool,
    pub command_buffer: vk::CommandBuffer,
    pub camera_buffer: AllocatedBuffer,
    pub global_descriptor_set: vk::DescriptorSet,
}

impl Frame {
    pub fn new(
        device: &ash::Device,
        allocator: &mut Allocator,
        graphics_family_index: u32,
        global_descriptor_set: vk::DescriptorSet,
    ) -> anyhow::Result<Self> {
        let (command_pool, command_buffer) =
            Self::create_commands(device, graphics_family_index)?;
        let (
            present_semaphore,
            render_semaphore,
            render_fence,
        ) = Self::create_sync_objs(device)?;
        let camera_buffer = AllocatedBuffer::new(
            device,
            allocator,
            std::mem::size_of::<CameraData>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            "Uniform Camera Buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        Ok(Self {
            present_semaphore,
            render_semaphore,
            render_fence,
            command_pool,
            command_buffer,
            camera_buffer,
            global_descriptor_set,
        })
    }

    pub fn cleanup(self, device: &ash::Device) {
        unsafe {
            device.destroy_semaphore(self.render_semaphore, None);
            device.destroy_semaphore(self.present_semaphore, None);
            device.destroy_fence(self.render_fence, None);
            device.destroy_command_pool(self.command_pool, None);
        }
    }

    fn create_commands(
        device: &ash::Device,
        graphics_family_index: u32,
    ) -> anyhow::Result<(vk::CommandPool, vk::CommandBuffer)> {
        let pool_info = vk::CommandPoolCreateInfo {
            queue_family_index: graphics_family_index,
            // Allow the pool to reset individual command buffers
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            ..Default::default()
        };
        let command_pool =
            unsafe { device.create_command_pool(&pool_info, None)? };

        let buffer_info = vk::CommandBufferAllocateInfo {
            command_pool,
            command_buffer_count: 1,
            level: vk::CommandBufferLevel::PRIMARY,
            ..Default::default()
        };
        let command_buffer =
            unsafe { device.allocate_command_buffers(&buffer_info)?[0] };

        Ok((command_pool, command_buffer))
    }

    fn create_sync_objs(
        device: &ash::Device
    ) -> anyhow::Result<(vk::Semaphore, vk::Semaphore, vk::Fence)> {
        let fence_info = vk::FenceCreateInfo {
            // Fence starts out signaled so we can wait on it for the first frame
            flags: vk::FenceCreateFlags::SIGNALED,
            ..Default::default()
        };
        let render_fence = unsafe {
            device.create_fence(&fence_info, None)?
        };

        let sem_info = vk::SemaphoreCreateInfo::default();
        let present_semaphore = unsafe {
            device.create_semaphore(&sem_info, None)?
        };
        let render_semaphore = unsafe {
            device.create_semaphore(&sem_info, None)?
        };

        Ok((present_semaphore, render_semaphore, render_fence))
    }

}

use ash::vk;
use gpu_allocator::vulkan::Allocator;
use color_eyre::eyre::Result;

use crate::renderer::memory::AllocatedBuffer;

use super::camera::GpuCameraData;

#[derive(Debug)]
pub struct Frame {
    pub present_semaphore: vk::Semaphore,
    pub render_semaphore: vk::Semaphore,
    pub render_fence: vk::Fence,
    pub command_pool: vk::CommandPool,
    pub command_buffer: vk::CommandBuffer,
    pub camera_buffer: AllocatedBuffer,
    pub descriptor_set: vk::DescriptorSet,
}

impl Frame {
    pub fn new(
        device: &ash::Device,
        allocator: &mut Allocator,
        graphics_family_index: u32,
        descriptor_pool: vk::DescriptorPool,
        descriptor_set_layout: vk::DescriptorSetLayout,
    ) -> Result<Self> {
        let (command_pool, command_buffer) =
            Self::create_commands(device, graphics_family_index)?;
        let (present_semaphore, render_semaphore, render_fence) =
            Self::create_sync_objs(device)?;
        let camera_buffer = AllocatedBuffer::new(
            device,
            allocator,
            std::mem::size_of::<GpuCameraData>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            "Uniform Camera Buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        // Allocate one descriptor set for this frame
        let descriptor_set = {
            let info = vk::DescriptorSetAllocateInfo {
                descriptor_pool,
                descriptor_set_count: 1,
                p_set_layouts: &descriptor_set_layout,
                ..Default::default()
            };
            unsafe { device.allocate_descriptor_sets(&info)?[0] }
        };

        // Point descriptor set to camera buffer
        {
            let binfo = vk::DescriptorBufferInfo {
                buffer: camera_buffer.buffer,
                offset: 0,
                range: std::mem::size_of::<GpuCameraData>() as u64,
            };

            let write = vk::WriteDescriptorSet {
                // Write into binding number 0
                dst_binding: 0,
                dst_set: descriptor_set,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                p_buffer_info: &binfo,
                ..Default::default()
            };

            unsafe { device.update_descriptor_sets(&[write], &[]) }
        }

        Ok(Self {
            present_semaphore,
            render_semaphore,
            render_fence,
            command_pool,
            command_buffer,
            camera_buffer,
            descriptor_set,
        })
    }

    pub fn copy_data_to_camera_buffer<T>(
        &mut self,
        data: &[T],
    ) -> Result<presser::CopyRecord>
    where
        T: Copy,
    {
        Ok(presser::copy_from_slice_to_offset(
            data,
            &mut self.camera_buffer.allocation,
            0,
        )?)
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            self.camera_buffer.cleanup(device, allocator);
            device.destroy_semaphore(self.render_semaphore, None);
            device.destroy_semaphore(self.present_semaphore, None);
            device.destroy_fence(self.render_fence, None);
            device.destroy_command_pool(self.command_pool, None);
        }
    }

    fn create_commands(
        device: &ash::Device,
        graphics_family_index: u32,
    ) -> Result<(vk::CommandPool, vk::CommandBuffer)> {
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

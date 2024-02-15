use ash::vk;
use color_eyre::eyre::{OptionExt, Result};
use gpu_allocator::vulkan::Allocator;

use crate::renderer::{
    core::Core, memory::AllocatedBuffer, vkinit, MAX_OBJECTS,
};

use super::{
    camera::GpuCameraData, object::GpuObjectData, scene::GpuSceneData,
};

#[derive(Debug)]
pub struct Frame {
    pub present_semaphore: vk::Semaphore,
    pub render_semaphore: vk::Semaphore,
    pub render_fence: vk::Fence,
    pub command_pool: vk::CommandPool,
    pub command_buffer: vk::CommandBuffer,

    pub global_desc_set: vk::DescriptorSet,
    pub object_desc_set: vk::DescriptorSet,

    pub object_buffer: AllocatedBuffer,
}

impl Frame {
    pub fn new(
        core: &mut Core,
        descriptor_pool: &vk::DescriptorPool,
        global_desc_set_layout: &vk::DescriptorSetLayout,
        object_desc_set_layout: &vk::DescriptorSetLayout,
        scene_camera_buffer: &AllocatedBuffer,
    ) -> Result<Self> {
        let device = &core.device;

        // Create command pool and command buffer
        let (command_pool, command_buffer) = {
            let graphics_family_index = core
                .queue_family_indices
                .graphics_family
                .ok_or_eyre("Graphics family index not found")?;
            Self::create_commands(device, graphics_family_index)?
        };

        // Create semaphores and fences
        let (present_semaphore, render_semaphore, render_fence) =
            Self::create_sync_objs(device)?;

        // Allocate descriptor set using the global descriptor set layout
        let global_desc_set = {
            let info = vk::DescriptorSetAllocateInfo {
                descriptor_pool: *descriptor_pool,
                descriptor_set_count: 1,
                p_set_layouts: global_desc_set_layout,
                ..Default::default()
            };
            unsafe { device.allocate_descriptor_sets(&info)?[0] }
        };

        // Allocate descriptor set using the object descriptor set layout
        let object_desc_set = {
            let info = vk::DescriptorSetAllocateInfo {
                descriptor_pool: *descriptor_pool,
                descriptor_set_count: 1,
                p_set_layouts: object_desc_set_layout,
                ..Default::default()
            };
            unsafe { device.allocate_descriptor_sets(&info)?[0] }
        };

        // Create object buffer
        let object_buffer = AllocatedBuffer::new(
            &core.device,
            &mut core.get_allocator()?,
            std::mem::size_of::<GpuObjectData>() as u64 * MAX_OBJECTS as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            "Object Storage Buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        {
            // Point descriptor set to the scene binding in the scene-camera buffer
            let scene_info = vk::DescriptorBufferInfo {
                buffer: scene_camera_buffer.buffer,
                offset: 0,
                range: std::mem::size_of::<GpuSceneData>() as u64,
            };
            // Point descriptor set to the camera binding in the scene-camera buffer
            let camera_info = vk::DescriptorBufferInfo {
                buffer: scene_camera_buffer.buffer,
                offset: 0,
                range: std::mem::size_of::<GpuCameraData>() as u64,
            };
            // Point descriptor set to object buffer
            let object_info = vk::DescriptorBufferInfo {
                buffer: object_buffer.buffer,
                offset: 0,
                range: std::mem::size_of::<GpuObjectData>() as u64
                    * MAX_OBJECTS as u64,
            };

            // Scene data is in binding 0
            let scene_write = vkinit::write_descriptor_set(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                global_desc_set,
                0,
                &scene_info,
            );
            // Camera data is in binding 1
            let camera_write = vkinit::write_descriptor_set(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                global_desc_set,
                1,
                &camera_info,
            );
            let object_write = vkinit::write_descriptor_set(
                vk::DescriptorType::STORAGE_BUFFER,
                object_desc_set,
                0,
                &object_info,
            );

            let writes = [scene_write, camera_write, object_write];
            unsafe { device.update_descriptor_sets(&writes, &[]) }
        }

        Ok(Self {
            present_semaphore,
            render_semaphore,
            render_fence,
            command_pool,
            command_buffer,
            global_desc_set,
            object_desc_set,
            object_buffer,
        })
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            self.object_buffer.cleanup(device, allocator);
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

use ash::vk;
use color_eyre::eyre::Result;
use gpu_allocator::vulkan::Allocator;

use crate::renderer::{
    buffer::AllocatedBuffer, core::Core, descriptors::DescriptorAllocator,
    inner::MAX_OBJECTS, vkinit,
};

use super::resources::{
    camera::GpuCameraData, object::GpuObjectData, scene::GpuSceneData,
};

#[derive(Debug)]
pub struct Frame {
    pub present_semaphore: vk::Semaphore,
    pub render_semaphore: vk::Semaphore,
    pub render_fence: vk::Fence,
    pub command_buffer: vk::CommandBuffer,

    pub global_desc_set: vk::DescriptorSet,
    pub object_desc_set: vk::DescriptorSet,

    pub object_buffer: AllocatedBuffer,
}

impl Frame {
    pub fn new(
        core: &mut Core,
        scene_camera_buffer: &AllocatedBuffer,
        command_pool: &vk::CommandPool,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<Self> {
        let device = &core.device;

        // Create command buffer
        let command_buffer = Self::create_command_buffer(device, command_pool)?;

        // Create semaphores and fences
        let (present_semaphore, render_semaphore, render_fence) =
            Self::create_sync_objs(device)?;

        // Allocate descriptor set using the global descriptor set layout
        let global_desc_set =
            desc_allocator.allocate(&core.device, "global")?;
        // Allocate descriptor set using the object descriptor set layout
        let object_desc_set =
            desc_allocator.allocate(&core.device, "object")?;

        // Create object buffer
        let mut allocator = core.get_allocator()?;
        let object_buffer = AllocatedBuffer::new(
            &core.device,
            &mut allocator,
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
            let scene_write = vkinit::write_descriptor_buffer(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                global_desc_set,
                0,
                &scene_info,
            );
            // Camera data is in binding 1
            let camera_write = vkinit::write_descriptor_buffer(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                global_desc_set,
                1,
                &camera_info,
            );
            let object_write = vkinit::write_descriptor_buffer(
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
use ash::vk;

use super::{core::Core, destruction_queue::Destroy};

pub struct Commands {
    pub command_pool: vk::CommandPool,
    pub main_command_buffer: vk::CommandBuffer,
}

impl Commands {
    pub fn new(core: &Core) -> anyhow::Result<Self> {
        let pool_info = vk::CommandPoolCreateInfo {
            queue_family_index: core
                .queue_family_indices
                .graphics_family
                .unwrap(),
            // Allow the pool to reset individual command buffers
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            ..Default::default()
        };
        let command_pool = unsafe {
            core.device.create_command_pool(&pool_info, None)?
        };

        let buffer_info = vk::CommandBufferAllocateInfo {
            command_pool,
            command_buffer_count: 1,
            level: vk::CommandBufferLevel::PRIMARY,
            ..Default::default()
        };
        let command_buffers = unsafe {
            core.device
                .allocate_command_buffers(&buffer_info)?
        };

        let objs = Self {
            command_pool,
            main_command_buffer: command_buffers[0],
        };

        Ok(objs)
    }
}

impl Destroy for Commands {
    fn destroy(&self, device: &ash::Device) {
        log::info!("Cleaning up command objects ...");
        unsafe {
            device.destroy_command_pool(self.command_pool, None);
        }
    }
}

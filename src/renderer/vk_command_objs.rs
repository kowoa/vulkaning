use ash::vk;

use super::vk_core_objs::VkCoreObjs;

pub struct VkCommandObjs {
    command_pool: vk::CommandPool,
    main_command_buffer: vk::CommandBuffer,
}

impl VkCommandObjs {
    pub fn new(core_objs: &VkCoreObjs) -> anyhow::Result<Self> {
        let pool_info = vk::CommandPoolCreateInfo {
            queue_family_index: core_objs
                .queue_family_indices
                .graphics_family
                .unwrap(),
            // Allow the pool to reset individual command buffers
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            ..Default::default()
        };
        let command_pool = unsafe {
            core_objs.device.create_command_pool(&pool_info, None)?
        };

        let buffer_info = vk::CommandBufferAllocateInfo {
            command_pool,
            command_buffer_count: 1,
            level: vk::CommandBufferLevel::PRIMARY,
            ..Default::default()
        };
        let command_buffers = unsafe {
            core_objs.device
                .allocate_command_buffers(&buffer_info)?
        };

        Ok(Self {
            command_pool,
            main_command_buffer: command_buffers[0],
        })
    }

    pub fn destroy(&mut self, core_objs: &VkCoreObjs) {
        unsafe {
            core_objs.device.destroy_command_pool(self.command_pool, None);
        }
    }
}

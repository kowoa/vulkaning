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
            core_objs.device.create_command_pool(&pool_info, None)?;
        }
    }
}

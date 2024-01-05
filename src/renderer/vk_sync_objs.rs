use std::rc::Rc;

use ash::vk;


use super::{vk_core_objs::VkCoreObjs, destruction_queue::{Destroy, self, DestructionQueue}};

pub struct VkSyncObjs {
    pub present_semaphore: vk::Semaphore,
    pub render_semaphore: vk::Semaphore,
    pub render_fence: vk::Fence,
}

impl VkSyncObjs {
    pub fn new(core_objs: &VkCoreObjs) -> anyhow::Result<Self> {
        let fence_info = vk::FenceCreateInfo {
            // Fence starts out signaled so we can wait on it for the first frame
            flags: vk::FenceCreateFlags::SIGNALED,
            ..Default::default()
        };
        let render_fence = unsafe {
            core_objs.device.create_fence(&fence_info, None)?
        };

        let sem_info = vk::SemaphoreCreateInfo::default();
        let present_semaphore = unsafe {
            core_objs.device.create_semaphore(&sem_info, None)?
        };
        let render_semaphore = unsafe {
            core_objs.device.create_semaphore(&sem_info, None)?
        };

        let objs = Self {
            present_semaphore,
            render_semaphore,
            render_fence,
        };

        Ok(objs)
    }
}

impl Destroy for VkSyncObjs {
    fn destroy(&self, device: &ash::Device) {
        log::info!("Cleaning up sync objects ...");
        unsafe {
            device.destroy_semaphore(self.render_semaphore, None);
            device.destroy_semaphore(self.present_semaphore, None);
            device.destroy_fence(self.render_fence, None);
        }
    }
}

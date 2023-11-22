mod init;
use init::*;

use ash::vk;

pub struct Renderer {
    vulkan_core: VulkanCore,
}

impl Renderer {
    pub fn new(
        window: &winit::window::Window,
        event_loop: &winit::event_loop::EventLoop<()>,
    ) -> anyhow::Result<Self> {
        log::info!("Initializing renderer ...");

        let vulkan_core = VulkanCore::new(event_loop)?;

        Ok(Self {
            vulkan_core
        })
    }

    pub fn draw_frame(&self) {
        log::info!("Drawing frame ...");
    }

    pub fn present_frame(&self) {
        log::info!("Presenting frame ...");
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.vulkan_core.destroy(None);
        }
    }
}

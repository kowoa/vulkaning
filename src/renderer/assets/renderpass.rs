use ash::vk;

use crate::renderer::swapchain::Swapchain;

pub struct Renderpass {
    pub renderpass: vk::RenderPass,
    pub framebuffers: Vec<vk::Framebuffer>,
}

impl Renderpass {
    pub fn new(
        device: &ash::Device,
        swapchain: &Swapchain,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> {
        let renderpass = create_renderpass(device, &swapchain.image_format)?;
        let framebuffers = create_framebuffers(
            &renderpass,
            device,
            &swapchain.image_views,
            window,
        )?;

        Ok(Self {
            renderpass,
            framebuffers,
        })
    }

    pub fn cleanup(self, device: &ash::Device) {
        log::info!("Cleaning up renderpass ...");
        unsafe {
            for framebuffer in &self.framebuffers {
                device.destroy_framebuffer(*framebuffer, None);
            }
            device.destroy_render_pass(self.renderpass, None);
        }
    }
}

fn create_renderpass(
    device: &ash::Device,
    swapchain_image_format: &vk::Format,
) -> anyhow::Result<vk::RenderPass> {
    // Description of the image we will be writing into with rendering commands
    let color_attachment = vk::AttachmentDescription {
        format: *swapchain_image_format,
        samples: vk::SampleCountFlags::TYPE_1,
        // Clear when this attachment is loaded
        load_op: vk::AttachmentLoadOp::CLEAR,
        // Keep attachment stored when renderpass ends
        store_op: vk::AttachmentStoreOp::STORE,
        // We don't care about stencil
        stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
        stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
        // We don't know or care about the starting layout of attachment
        initial_layout: vk::ImageLayout::UNDEFINED,
        // After the renderpass ends, the image has to be in a layout ready for display
        final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        ..Default::default()
    };

    let color_attachment_ref = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    };

    let subpass = vk::SubpassDescription {
        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        color_attachment_count: 1,
        p_color_attachments: &color_attachment_ref,
        ..Default::default()
    };

    let renderpass_info = vk::RenderPassCreateInfo {
        attachment_count: 1,
        p_attachments: &color_attachment,
        subpass_count: 1,
        p_subpasses: &subpass,
        ..Default::default()
    };

    Ok(unsafe { device.create_render_pass(&renderpass_info, None)? })
}

fn create_framebuffers(
    renderpass: &vk::RenderPass,
    device: &ash::Device,
    swapchain_image_views: &Vec<vk::ImageView>,
    window: &winit::window::Window,
) -> anyhow::Result<Vec<vk::Framebuffer>> {
    Ok(swapchain_image_views
        .iter()
        .map(|view| {
            let fb_info = vk::FramebufferCreateInfo {
                render_pass: *renderpass,
                attachment_count: 1,
                width: window.inner_size().width,
                height: window.inner_size().height,
                layers: 1,
                p_attachments: view,
                ..Default::default()
            };

            unsafe { device.create_framebuffer(&fb_info, None) }
        })
        .collect::<Result<Vec<_>, _>>()?)
}

use ash::vk;

use super::{vk_core_objs::VkCoreObjs, vk_swapchain_objs::VkSwapchainObjs};

pub struct VkRenderpassObjs {
    renderpass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
}

impl VkRenderpassObjs {
    pub fn new(
        core_objs: &VkCoreObjs,
        swapchain_objs: &VkSwapchainObjs,
    ) -> anyhow::Result<Self> {
        let renderpass = create_renderpass(core_objs)?;
        let framebuffers = create_framebuffers(core_objs, swapchain_objs)?;
        Ok(Self {
            renderpass,
            framebuffers,
        })
    }
}

fn create_renderpass(core_objs: &VkCoreObjs) -> anyhow::Result<vk::RenderPass> {
    // Description of the image we will be writing into with rendering commands
    let color_attachment = vk::AttachmentDescription {
        format: swapchain_objs.swapchain_image_format,
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

    Ok(unsafe {
        core_objs
            .device
            .create_render_pass(&renderpass_info, None)?
    })
}

fn create_framebuffers(
    renderpass: &vk::RenderPass,
    core_objs: &VkCoreObjs,
    swapchain_objs: &VkSwapchainObjs,
    window: &winit::window::Window,
) -> anyhow::Result<Vec<vk::Framebuffer>> {
    swapchain_objs
        .swapchain_image_views
        .iter()
        .map(|view| {
            let fb_info = vk::FramebufferCreateInfo {
                render_pass: renderpass,
                attachment_count: 1,
                width: window.inner_size().width,
                height: window.inner_size().height,
                layers: 1,
                p_attachments: view.as_ptr(),
                ..Default::default()
            };

            unsafe {
                core_objs
                    .device
                    .create_framebuffer(&fb_info, None)
            }
        })
        .collect::<Result<Vec<_>, _>>()
}

use ash::vk;
use color_eyre::eyre::Result;

use crate::renderer::swapchain::Swapchain;

pub struct Renderpass {
    pub renderpass: vk::RenderPass,
    pub framebuffers: Vec<vk::Framebuffer>,
}

impl Renderpass {
    pub fn new(
        device: &ash::Device,
        swapchain: &Swapchain,
    ) -> Result<Self> {
        let renderpass = create_renderpass(device, swapchain)?;
        let framebuffers = create_framebuffers(&renderpass, device, swapchain)?;

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
    swapchain: &Swapchain,
) -> Result<vk::RenderPass> {
    let attachments = [
        // Color attachment (where rendering commands will be written into)
        vk::AttachmentDescription {
            format: swapchain.image_format,
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
            //final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            ..Default::default()
        },
        // Depth attachment
        vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(),
            format: swapchain.depth_image.image_format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::CLEAR,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        }
    ];
    
    let color_attachment_ref = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    };

    let depth_attachment_ref = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };

    let subpass = vk::SubpassDescription {
        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        color_attachment_count: 1,
        p_color_attachments: &color_attachment_ref,
        p_depth_stencil_attachment: &depth_attachment_ref,
        ..Default::default()
    };

    let color_dependency = vk::SubpassDependency {
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        src_access_mask: vk::AccessFlags::empty(),
        dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ..Default::default()
    };

    let depth_dependency = vk::SubpassDependency {
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
        src_access_mask: vk::AccessFlags::empty(),
        dst_stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
        dst_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ..Default::default()
    };

    let dependencies = [color_dependency, depth_dependency];

    let renderpass_info = vk::RenderPassCreateInfo {
        attachment_count: 2,
        p_attachments: attachments.as_ptr(),
        subpass_count: 1,
        p_subpasses: &subpass,
        dependency_count: 2,
        p_dependencies: dependencies.as_ptr(),
        ..Default::default()
    };

    Ok(unsafe { device.create_render_pass(&renderpass_info, None)? })
}

fn create_framebuffers(
    renderpass: &vk::RenderPass,
    device: &ash::Device,
    swapchain: &Swapchain,
) -> Result<Vec<vk::Framebuffer>> {
    Ok(swapchain
        .image_views
        .iter()
        .map(|view| {
            let attachments = [*view, swapchain.depth_image.image_view];
            let fb_info = vk::FramebufferCreateInfo {
                render_pass: *renderpass,
                width: swapchain.image_extent.width,
                height: swapchain.image_extent.height,
                layers: 1,
                p_attachments: attachments.as_ptr(),
                attachment_count: attachments.len() as u32,
                ..Default::default()
            };

            unsafe { device.create_framebuffer(&fb_info, None) }
        })
        .collect::<Result<Vec<_>, _>>()?)
}

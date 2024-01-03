use std::ffi::CString;

use anyhow::anyhow;
use ash::vk;

use super::{
    shader::Shader, vk_core_objs::VkCoreObjs, vk_initializers,
    vk_renderpass_objs::VkRenderpassObjs, vk_swapchain_objs::VkSwapchainObjs,
};

pub struct VkPipelineObjs {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
}

impl VkPipelineObjs {
    pub fn new(
        core_objs: &VkCoreObjs,
        swapchain_objs: &VkSwapchainObjs,
        renderpass_objs: &VkRenderpassObjs,
    ) -> anyhow::Result<Self> {
        let shader = Shader::new("triangle")?;
        let shader_mod_vert =
            shader.create_shader_module_vert(&core_objs.device)?;
        let shader_mod_frag =
            shader.create_shader_module_frag(&core_objs.device)?;

        let pipeline_info = PipelineInfo::new(
            &shader_mod_vert,
            &shader_mod_frag,
            core_objs,
            swapchain_objs,
        );
        let pipeline_layout = create_pipeline_layout(core_objs)?;
        let pipeline =
            create_pipeline(core_objs, renderpass_objs, &pipeline_layout, &pipeline_info)?;

        unsafe {
            core_objs.device.destroy_shader_module(shader_mod_vert, None);
            core_objs.device.destroy_shader_module(shader_mod_frag, None);
        }

        Ok(Self {
            pipeline,
            pipeline_layout,
        })
    }

    pub fn destroy(&mut self, core_objs: &VkCoreObjs) {
        log::info!("Cleaning up pipeline objects ...");
        unsafe {
            core_objs.device.destroy_pipeline(self.pipeline, None);
            core_objs.device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}

#[derive(Debug)]
struct PipelineInfo {
    shader_main_fn_name: CString,
    shader_stages: Vec<vk::PipelineShaderStageCreateInfo>,
    vertex_input: vk::PipelineVertexInputStateCreateInfo,
    input_assembly: vk::PipelineInputAssemblyStateCreateInfo,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
    rasterizer: vk::PipelineRasterizationStateCreateInfo,
    color_blend_attachment: vk::PipelineColorBlendAttachmentState,
    multisampling: vk::PipelineMultisampleStateCreateInfo,
}

impl PipelineInfo {
    pub fn new(
        shader_mod_vert: &vk::ShaderModule,
        shader_mod_frag: &vk::ShaderModule,
        core_objs: &VkCoreObjs,
        swapchain_objs: &VkSwapchainObjs,
    ) -> Self {
        use vk_initializers as vkinit;

        let shader_main_fn_name = CString::new("main").unwrap();
        let shader_stages = vec![
            vkinit::pipeline_shader_stage_create_info(
                vk::ShaderStageFlags::VERTEX,
                *shader_mod_vert,
                &shader_main_fn_name,
            ),
            vkinit::pipeline_shader_stage_create_info(
                vk::ShaderStageFlags::FRAGMENT,
                *shader_mod_frag,
                &shader_main_fn_name,
            ),
        ];
        let vertex_input = vkinit::vertex_input_state_create_info();
        let input_assembly = vkinit::input_assembly_create_info(
            vk::PrimitiveTopology::TRIANGLE_LIST,
        );
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: swapchain_objs.swapchain_extent.width as f32,
            height: swapchain_objs.swapchain_extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain_objs.swapchain_extent,
        };
        let rasterizer =
            vkinit::rasterization_state_create_info(vk::PolygonMode::FILL);
        let color_blend_attachment = vkinit::color_blend_attachment_state();
        let multisampling = vkinit::multisampling_state_create_info();

        Self {
            shader_main_fn_name,
            shader_stages,
            vertex_input,
            input_assembly,
            viewport,
            scissor,
            rasterizer,
            color_blend_attachment,
            multisampling,
        }
    }
}

fn create_pipeline(
    core_objs: &VkCoreObjs,
    renderpass_objs: &VkRenderpassObjs,
    pipeline_layout: &vk::PipelineLayout,
    info: &PipelineInfo,
) -> anyhow::Result<vk::Pipeline> {
    let viewport_state_info = vk::PipelineViewportStateCreateInfo {
        viewport_count: 1,
        p_viewports: &info.viewport,
        scissor_count: 1,
        p_scissors: &info.scissor,
        ..Default::default()
    };

    let color_blend_info = vk::PipelineColorBlendStateCreateInfo {
        logic_op_enable: vk::FALSE,
        logic_op: vk::LogicOp::COPY,
        attachment_count: 1,
        p_attachments: &info.color_blend_attachment,
        ..Default::default()
    };

    let pipeline_create_infos = [vk::GraphicsPipelineCreateInfo {
        flags: vk::PipelineCreateFlags::empty(),
        stage_count: info.shader_stages.len() as u32,
        p_stages: info.shader_stages.as_ptr(),
        p_vertex_input_state: &info.vertex_input,
        p_input_assembly_state: &info.input_assembly,
        p_viewport_state: &viewport_state_info,
        p_rasterization_state: &info.rasterizer,
        p_multisample_state: &info.multisampling,
        p_color_blend_state: &color_blend_info,
        layout: *pipeline_layout,
        render_pass: renderpass_objs.renderpass,
        subpass: 0,
        base_pipeline_handle: vk::Pipeline::null(),
        p_tessellation_state: std::ptr::null(),
        ..Default::default()
    }];

    let graphics_pipelines = unsafe {
        match core_objs.device.create_graphics_pipelines(
            vk::PipelineCache::null(),
            &pipeline_create_infos,
            None,
        ) {
            Ok(pipelines) => Ok(pipelines),
            Err(_) => Err(anyhow!("Failed to create graphics piplines")),
        }
    }?;
    Ok(graphics_pipelines[0])
}

fn create_pipeline_layout(
    core_objs: &VkCoreObjs,
) -> anyhow::Result<vk::PipelineLayout> {
    // Build the pipeline layout that controls the inputs/outputs of the shader
    let layout_info = vk_initializers::pipeline_layout_create_info();
    Ok(unsafe {
        core_objs
            .device
            .create_pipeline_layout(&layout_info, None)?
    })
}

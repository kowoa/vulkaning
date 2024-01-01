use ash::vk;

use super::{
    vk_core_objs::VkCoreObjs, vk_initializers,
    vk_renderpass_objs::VkRenderpassObjs, shader::Shader,
};

pub struct VkPipelineObjs {
    shader_mod_vert: vk::ShaderModule,
    shader_mod_frag: vk::ShaderModule,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
}

impl VkPipelineObjs {
    pub fn new(
        core_objs: &VkCoreObjs,
        renderpass_objs: &VkRenderpassObjs,
    ) -> anyhow::Result<Self> {
        let shader = Shader::new("triangle")?;
        let shader_mod_vert = shader.create_shader_module_vert(&core_objs.device)?;
        let shader_mod_frag = shader.create_shader_module_frag(&core_objs.device)?;
        
        let pipeline_info = PipelineInfo::new(&shader_mod_vert, shader_mod_frag);
        let pipeline =
            create_pipeline(core_objs, renderpass_objs, &pipeline_info)?;
        let pipeline_layout = create_pipeline_layout(core_objs)?;

        Ok(Self {
            shader_mod_vert,
            shader_mod_frag,
            pipeline,
            pipeline_layout,
        })
    }
}

struct PipelineInfo {
    shader_stages: Vec<vk::PipelineShaderStageCreateInfo>,
    vertex_input: vk::PipelineVertexInputStateCreateInfo,
    input_assembly: vk::PipelineInputAssemblyStateCreateInfo,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
    rasterizer: vk::PipelineRasterizationStateCreateInfo,
    color_blend_attachment: vk::PipelineColorBlendAttachmentState,
    multisampling: vk::PipelineMultisampleStateCreateInfo,
    pipeline_layout: vk::PipelineLayout,
}

impl PipelineInfo {
    pub fn new() -> Self {
        let shader_stages = Vec::new();
        shader_stages.push(vk_initializers::pipeline_shader_stage_create_info(vk::ShaderStageFlags::VERTEX, vertex_shader));
    }
}

fn create_pipeline(
    core_objs: &VkCoreObjs,
    renderpass_objs: &VkRenderpassObjs,
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
        p_attachments: info.color_blend_attachment,
        ..Default::default()
    };

    let pipeline_info = vk::GraphicsPipelineCreateInfo {
        stage_count: info.shader_stages.len(),
        p_stages: info.shader_stages,
        p_vertex_input_state: info.vertex_input,
        p_input_assembly_state: info.input_assembly,
        p_viewport_state: viewport_state_info,
        p_rasterization_state: info.rasterizer,
        p_multisample_state: info.multisampling,
        p_color_blend_state: color_blend_info,
        layout: info.pipeline_layout,
        render_pass: renderpass_objs.renderpass,
        subpass: 0,
        base_pipeline_handle: vk::Pipeline::null(),
        ..Default::default()
    };

    Ok(unsafe {
        core_objs.device.create_graphics_pipelines(
            vk::PipelineCache::null(),
            &[pipeline_info],
            None,
        )?
    })
}

fn create_pipeline_layout(
    core_objs: &VkCoreObjs,
) -> anyhow::Result<vk::PipelineLayout> {
    // Build the pipeline layout that controls the inputs/outputs of the shader
    let layout_info = vk_initializers::pipeline_layout_create_info();
    Ok(unsafe { core_objs.device.create_pipeline_layout(&layout_info, None) })
}

use std::ffi::CString;

use anyhow::anyhow;
use ash::vk;

use crate::renderer::{swapchain::Swapchain, vk_initializers};

use super::vertex::VertexInputDescription;

pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
}

impl Pipeline {
    pub fn cleanup(self, device: &ash::Device) {
        log::info!("Cleaning up pipeline ...");
        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_pipeline(self.pipeline, None);
        }
    }
}

pub struct PipelineBuilder {
    _shader_main_fn_name: CString,
    shader_stages: Vec<vk::PipelineShaderStageCreateInfo>,
    vertex_input: vk::PipelineVertexInputStateCreateInfo,
    _vertex_input_desc: Option<VertexInputDescription>,
    input_assembly: vk::PipelineInputAssemblyStateCreateInfo,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
    rasterizer: vk::PipelineRasterizationStateCreateInfo,
    color_blend_attachment: vk::PipelineColorBlendAttachmentState,
    multisampling: vk::PipelineMultisampleStateCreateInfo,
    pipeline_layout: vk::PipelineLayout,
    depth_stencil: vk::PipelineDepthStencilStateCreateInfo,
}

impl PipelineBuilder {
    pub fn new(
        vert_shader_mod: &vk::ShaderModule,
        frag_shader_mod: &vk::ShaderModule,
        device: &ash::Device,
        swapchain: &Swapchain,
    ) -> anyhow::Result<Self> {
        use vk_initializers as vkinit;

        let shader_main_fn_name = CString::new("main").unwrap();
        let shader_stages = vec![
            vkinit::pipeline_shader_stage_create_info(
                vk::ShaderStageFlags::VERTEX,
                *vert_shader_mod,
                &shader_main_fn_name,
            ),
            vkinit::pipeline_shader_stage_create_info(
                vk::ShaderStageFlags::FRAGMENT,
                *frag_shader_mod,
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
            width: swapchain.extent.width as f32,
            height: swapchain.extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain.extent,
        };
        let rasterizer =
            vkinit::rasterization_state_create_info(vk::PolygonMode::FILL);
        let color_blend_attachment = vkinit::color_blend_attachment_state();
        let multisampling = vkinit::multisampling_state_create_info();
        let pipeline_layout = default_pipeline_layout(device)?;
        let depth_stencil = vkinit::depth_stencil_create_info(
            true,
            true,
            vk::CompareOp::LESS_OR_EQUAL,
        );

        Ok(Self {
            _shader_main_fn_name: shader_main_fn_name,
            shader_stages,
            vertex_input,
            _vertex_input_desc: None,
            input_assembly,
            viewport,
            scissor,
            rasterizer,
            color_blend_attachment,
            multisampling,
            pipeline_layout,
            depth_stencil,
        })
    }

    pub fn pipeline_layout(
        mut self,
        layout: vk::PipelineLayout,
        device: &ash::Device,
    ) -> Self {
        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
        self.pipeline_layout = layout;
        self
    }

    pub fn shader_stages(
        mut self,
        stages: Vec<vk::PipelineShaderStageCreateInfo>,
    ) -> Self {
        self.shader_stages = stages;
        self
    }

    pub fn vertex_input(mut self, desc: VertexInputDescription) -> Self {
        self.vertex_input = vk::PipelineVertexInputStateCreateInfo {
            p_vertex_attribute_descriptions: desc.attributes.as_ptr(),
            vertex_attribute_description_count: desc.attributes.len() as u32,
            p_vertex_binding_descriptions: desc.bindings.as_ptr(),
            vertex_binding_description_count: desc.bindings.len() as u32,
            flags: desc.flags,
            ..Default::default()
        };
        // Need to store description else the pointers will be invalid
        self._vertex_input_desc = Some(desc);
        self
    }

    pub fn build(
        self,
        device: &ash::Device,
        renderpass: vk::RenderPass,
    ) -> anyhow::Result<Pipeline> {
        let viewport_state_info = vk::PipelineViewportStateCreateInfo {
            viewport_count: 1,
            p_viewports: &self.viewport,
            scissor_count: 1,
            p_scissors: &self.scissor,
            ..Default::default()
        };

        let color_blend_info = vk::PipelineColorBlendStateCreateInfo {
            logic_op_enable: vk::FALSE,
            logic_op: vk::LogicOp::COPY,
            attachment_count: 1,
            p_attachments: &self.color_blend_attachment,
            ..Default::default()
        };

        let pipeline_create_infos = [vk::GraphicsPipelineCreateInfo {
            flags: vk::PipelineCreateFlags::empty(),
            stage_count: self.shader_stages.len() as u32,
            p_stages: self.shader_stages.as_ptr(),
            p_vertex_input_state: &self.vertex_input,
            p_input_assembly_state: &self.input_assembly,
            p_viewport_state: &viewport_state_info,
            p_rasterization_state: &self.rasterizer,
            p_multisample_state: &self.multisampling,
            p_color_blend_state: &color_blend_info,
            layout: self.pipeline_layout,
            render_pass: renderpass,
            subpass: 0,
            base_pipeline_handle: vk::Pipeline::null(),
            p_tessellation_state: std::ptr::null(),
            p_depth_stencil_state: &self.depth_stencil,
            ..Default::default()
        }];

        let graphics_pipelines = unsafe {
            match device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &pipeline_create_infos,
                None,
            ) {
                Ok(pipelines) => Ok(pipelines),
                Err(_) => Err(anyhow!("Failed to create graphics piplines")),
            }
        }?;

        Ok(Pipeline {
            pipeline: graphics_pipelines[0],
            pipeline_layout: self.pipeline_layout,
        })
    }
}

fn default_pipeline_layout(
    device: &ash::Device,
) -> anyhow::Result<vk::PipelineLayout> {
    // Build the pipeline layout that controls the inputs/outputs of the shader
    let layout_info = vk_initializers::pipeline_layout_create_info();
    Ok(unsafe { device.create_pipeline_layout(&layout_info, None)? })
}

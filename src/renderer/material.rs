use color_eyre::eyre::{eyre, OptionExt, Result};
use std::ffi::CString;

use ash::vk;

use crate::renderer::{swapchain::Swapchain, vkinit};

use super::resources::vertex::VertexInputDescription;

#[derive(PartialEq)]
pub struct Material {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
}

impl Material {
    pub fn cleanup(self, device: &ash::Device) {
        log::info!("Cleaning up pipeline ...");
        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_pipeline(self.pipeline, None);
        }
    }
}

pub struct MaterialBuilder<'a> {
    // vk::Pipeline related info
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
    depth_stencil: vk::PipelineDepthStencilStateCreateInfo,

    // vk::PipelineLayout related info
    pipeline_layout: Option<vk::PipelineLayout>,

    device: &'a ash::Device,
}

impl<'a> MaterialBuilder<'a> {
    pub fn new(
        vert_shader_mod: &vk::ShaderModule,
        frag_shader_mod: &vk::ShaderModule,
        device: &'a ash::Device,
        swapchain: &Swapchain,
    ) -> Result<Self> {
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
            width: swapchain.image_extent.width as f32,
            height: swapchain.image_extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain.image_extent,
        };
        let rasterizer =
            vkinit::rasterization_state_create_info(vk::PolygonMode::FILL);
        let color_blend_attachment = vkinit::color_blend_attachment_state();
        let multisampling = vkinit::multisampling_state_create_info();
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
            depth_stencil,

            pipeline_layout: None,

            device,
        })
    }

    pub fn pipeline_layout(mut self, layout: vk::PipelineLayout) -> Self {
        self.pipeline_layout = Some(layout);
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
        mut self,
        device: &ash::Device,
        renderpass: vk::RenderPass,
    ) -> Result<Material> {
        let pipeline_layout = self
            .pipeline_layout
            .take()
            .ok_or_eyre("No pipeline layout provided for MaterialBuilder")?;

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
            layout: pipeline_layout,
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
                Err(_) => Err(eyre!("Failed to create graphics piplines")),
            }
        }?;

        Ok(Material {
            pipeline: graphics_pipelines[0],
            pipeline_layout,
        })
    }
}

impl<'a> Drop for MaterialBuilder<'a> {
    fn drop(&mut self) {
        // Destroy pipeline layout in case it was never used
        if let Some(layout) = self.pipeline_layout {
            unsafe {
                self.device.destroy_pipeline_layout(layout, None);
            }
        }
    }
}

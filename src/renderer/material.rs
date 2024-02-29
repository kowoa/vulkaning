use color_eyre::eyre::{eyre, OptionExt, Result};
use std::ffi::CString;

use ash::vk;

use super::{shader::Shader, vertex::VertexInputDescription};

#[derive(PartialEq, Clone)]
pub struct Material {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
}

impl Material {
    pub fn new(
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
    ) -> Self {
        Self {
            pipeline,
            pipeline_layout,
        }
    }

    pub fn builder<'a>(device: &'a ash::Device) -> MaterialBuilder<'a> {
        MaterialBuilder::new(device)
    }

    pub fn cleanup(self, device: &ash::Device) {
        log::info!("Cleaning up pipeline ...");
        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_pipeline(self.pipeline, None);
        }
    }
}

pub struct MaterialBuilder<'a> {
    device: &'a ash::Device,

    vertex_input_desc: VertexInputDescription,
    vertex_input: vk::PipelineVertexInputStateCreateInfo,
    input_assembly: vk::PipelineInputAssemblyStateCreateInfo,
    rasterization: vk::PipelineRasterizationStateCreateInfo,
    color_blend_attachment: vk::PipelineColorBlendAttachmentState,
    multisample: vk::PipelineMultisampleStateCreateInfo,
    depth_stencil: vk::PipelineDepthStencilStateCreateInfo,
    color_attachment_format: vk::Format,
    rendering_info: vk::PipelineRenderingCreateInfo,
    shader: Option<Shader>,
    pipeline_layout: Option<vk::PipelineLayout>,
}

impl<'a> MaterialBuilder<'a> {
    fn new(device: &'a ash::Device) -> Self {
        let vertex_input_desc = VertexInputDescription::default();
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&vertex_input_desc.attributes)
            .vertex_binding_descriptions(&vertex_input_desc.bindings)
            .build();
        let input_assembly = Self::default_input_assembly_info();
        let rasterization = Self::default_rasterization_info();
        let color_blend_attachment = Self::default_color_blend_state();
        let multisample = Self::default_multisample_info();
        let depth_stencil = Self::default_depth_stencil_info();
        let color_attachment_format = vk::Format::UNDEFINED;
        let rendering_info = vk::PipelineRenderingCreateInfo::default();
        let shader = None;
        let pipeline_layout = None;

        Self {
            device,

            vertex_input_desc,
            vertex_input,
            input_assembly,
            rasterization,
            color_blend_attachment,
            multisample,
            depth_stencil,
            color_attachment_format,
            rendering_info,
            shader,
            pipeline_layout,
        }
    }

    pub fn shader(mut self, shader: Shader) -> Self {
        let old_shader = self.shader.replace(shader);
        if let Some(shader) = old_shader {
            shader.cleanup(self.device);
        }
        self
    }

    pub fn pipeline_layout(mut self, layout: vk::PipelineLayout) -> Self {
        let old_layout = self.pipeline_layout.replace(layout);
        if let Some(layout) = old_layout {
            unsafe {
                self.device.destroy_pipeline_layout(layout, None);
            }
        }
        self
    }

    pub fn input_topology(mut self, topology: vk::PrimitiveTopology) -> Self {
        self.input_assembly.topology = topology;
        self.input_assembly.primitive_restart_enable = vk::FALSE;
        self
    }

    pub fn polygon_mode(mut self, mode: vk::PolygonMode) -> Self {
        self.rasterization.polygon_mode = mode;
        self.rasterization.line_width = 1.0;
        self
    }

    pub fn cull_mode(
        mut self,
        cull_mode: vk::CullModeFlags,
        front_face: vk::FrontFace,
    ) -> Self {
        self.rasterization.cull_mode = cull_mode;
        self.rasterization.front_face = front_face;
        self
    }

    pub fn disable_multisampling(mut self) -> Self {
        self.multisample.sample_shading_enable = vk::FALSE;
        // 1 sample per pixel means no multisampling
        self.multisample.rasterization_samples = vk::SampleCountFlags::TYPE_1;
        self.multisample.min_sample_shading = 1.0;
        self.multisample.p_sample_mask = std::ptr::null();
        self.multisample.alpha_to_coverage_enable = vk::FALSE;
        self.multisample.alpha_to_one_enable = vk::FALSE;
        self
    }

    pub fn disable_blending(mut self) -> Self {
        // Default RGBA write mask
        self.color_blend_attachment.color_write_mask =
            vk::ColorComponentFlags::RGBA;
        // No blending
        self.color_blend_attachment.blend_enable = vk::FALSE;
        self
    }

    pub fn color_attachment_format(mut self, format: vk::Format) -> Self {
        self.color_attachment_format = format;
        // Connect the format to the rendering_info struct
        self.rendering_info.color_attachment_count = 1;
        self.rendering_info.p_color_attachment_formats =
            &self.color_attachment_format;
        self
    }

    pub fn depth_attachment_format(mut self, format: vk::Format) -> Self {
        self.rendering_info.depth_attachment_format = format;
        self
    }

    pub fn depth_test_enable(mut self, enable: bool) -> Self {
        self.depth_stencil.depth_test_enable =
            if enable { vk::TRUE } else { vk::FALSE };
        self.depth_stencil.depth_write_enable =
            if enable { vk::TRUE } else { vk::FALSE };
        self.depth_stencil.depth_compare_op = if enable {
            vk::CompareOp::LESS_OR_EQUAL
        } else {
            vk::CompareOp::ALWAYS
        };
        self.depth_stencil.min_depth_bounds = 0.0;
        self.depth_stencil.max_depth_bounds = 1.0;
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
        self.vertex_input_desc = desc;
        self
    }

    pub fn build(mut self) -> Result<Material> {
        let device = self.device;

        let shader = self
            .shader
            .take()
            .ok_or_eyre("No shader provided for MaterialBuilder")?;
        let shader_main_fn_name = CString::new("main").unwrap();
        let shader_stages = vec![
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(shader.vert_shader_mod)
                .name(&shader_main_fn_name)
                .build(),
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(shader.frag_shader_mod)
                .name(&shader_main_fn_name)
                .build(),
        ];

        let pipeline_layout = self
            .pipeline_layout
            .take()
            .ok_or_eyre("No pipeline layout provided for MaterialBuilder")?;

        let viewport_state = vk::PipelineViewportStateCreateInfo {
            viewport_count: 1,
            scissor_count: 1,
            ..Default::default()
        };

        let color_blend_info = vk::PipelineColorBlendStateCreateInfo {
            logic_op_enable: vk::FALSE,
            logic_op: vk::LogicOp::COPY,
            attachment_count: 1,
            p_attachments: &self.color_blend_attachment,
            ..Default::default()
        };

        // Use dynamic state for viewport and scissor configuration
        let dynamic_states =
            [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_info = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&dynamic_states)
            .build();

        let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .push_next(&mut self.rendering_info)
            .stages(&shader_stages)
            .layout(pipeline_layout)
            .vertex_input_state(&self.vertex_input)
            .input_assembly_state(&self.input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&self.rasterization)
            .multisample_state(&self.multisample)
            .color_blend_state(&color_blend_info)
            .depth_stencil_state(&self.depth_stencil)
            .dynamic_state(&dynamic_info)
            .build();

        let graphics_pipelines = unsafe {
            match device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[pipeline_info],
                None,
            ) {
                Ok(pipelines) => Ok(pipelines),
                Err(_) => Err(eyre!("Failed to create graphics pipelines")),
            }
        }?;

        shader.cleanup(device);

        Ok(Material {
            pipeline: graphics_pipelines[0],
            pipeline_layout,
        })
    }

    fn default_input_assembly_info() -> vk::PipelineInputAssemblyStateCreateInfo
    {
        vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false)
            .build()
    }

    fn default_rasterization_info() -> vk::PipelineRasterizationStateCreateInfo
    {
        vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            // Discards all primitives before rasterization stage if true
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            // Backface culling
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            // No depth bias
            .depth_bias_enable(false)
            .depth_bias_constant_factor(0.0)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0)
            .build()
    }

    fn default_color_blend_state() -> vk::PipelineColorBlendAttachmentState {
        vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)
            .build()
    }

    fn default_multisample_info() -> vk::PipelineMultisampleStateCreateInfo {
        vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            // 1 sample per pixel means no multisampling
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0)
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false)
            .build()
    }

    fn default_depth_stencil_info() -> vk::PipelineDepthStencilStateCreateInfo {
        vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .depth_bounds_test_enable(false)
            .min_depth_bounds(0.0)
            .max_depth_bounds(1.0)
            .stencil_test_enable(false)
            .build()
    }
}

impl<'a> Drop for MaterialBuilder<'a> {
    fn drop(&mut self) {
        // Destroy pipeline layout in case it was never used
        if let Some(layout) = self.pipeline_layout.take() {
            unsafe {
                self.device.destroy_pipeline_layout(layout, None);
            }
        }

        // Destroy shader in case it was never used
        if let Some(shader) = self.shader.take() {
            shader.cleanup(self.device);
        }
    }
}

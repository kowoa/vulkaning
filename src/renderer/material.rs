use bevy::log;
use color_eyre::eyre::{eyre, OptionExt, Result};
use glam::Vec4;
use std::{collections::HashMap, ffi::CString, sync::Arc};

use ash::vk;

use super::{
    context::Context,
    descriptors::{DescriptorSetLayoutBuilder, DescriptorWriter},
    gpu_data::GpuDrawPushConstants,
    image::AllocatedImage,
    shader::{ComputeShader, GraphicsShader},
    swapchain::Swapchain,
    vertex::VertexInputDescription,
};

pub struct MaterialInstance {
    pub material_name: String,
    pub desc_set: vk::DescriptorSet,
    pub pass: MaterialPass,
}

pub enum MaterialPass {
    Opaque,
    Transparent,
    Other,
}

#[derive(PartialEq, Clone)]
pub struct Material {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    pipeline_bind_point: vk::PipelineBindPoint,
}

impl Material {
    pub fn builder_graphics(
        device: &ash::Device,
    ) -> GraphicsMaterialBuilder<'_> {
        GraphicsMaterialBuilder::new(device)
    }

    pub fn builder_compute(device: &ash::Device) -> ComputeMaterialBuilder<'_> {
        ComputeMaterialBuilder::new(device)
    }

    pub fn cleanup(self, device: &ash::Device) {
        log::info!("Cleaning up pipeline ...");
        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_pipeline(self.pipeline, None);
        }
    }

    pub fn update_push_constants(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
        shader_stages: vk::ShaderStageFlags,
        constants: &[u8],
    ) {
        unsafe {
            device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                shader_stages,
                0,
                constants,
            );
        }
    }

    pub fn bind_pipeline(&self, cmd: vk::CommandBuffer, device: &ash::Device) {
        unsafe {
            device.cmd_bind_pipeline(
                cmd,
                self.pipeline_bind_point,
                self.pipeline,
            );
        }
    }

    pub fn bind_desc_sets(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
        first_set: u32,
        desc_sets: &[vk::DescriptorSet],
        dynamic_offsets: &[u32],
    ) {
        unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                self.pipeline_bind_point,
                self.pipeline_layout,
                first_set,
                desc_sets,
                dynamic_offsets,
            );
        }
    }
}

pub struct GraphicsMaterialBuilder<'a> {
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
    shader: Option<GraphicsShader>,
    pipeline_layout: Option<vk::PipelineLayout>,

    desc_sets: Vec<vk::DescriptorSet>,
}

impl<'a> GraphicsMaterialBuilder<'a> {
    fn new(device: &'a ash::Device) -> Self {
        let vertex_input_desc = VertexInputDescription::default();
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&vertex_input_desc.attributes)
            .vertex_binding_descriptions(&vertex_input_desc.bindings)
            .flags(vertex_input_desc.flags)
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

            desc_sets: Vec::new(),
        }
    }

    pub fn shader(mut self, shader: GraphicsShader) -> Self {
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

    // Make sure the transparent object is rendered AFTER the opaque ones
    pub fn enable_alpha_blending(mut self) -> Self {
        let blend = &mut self.color_blend_attachment;
        blend.color_write_mask = vk::ColorComponentFlags::RGBA;
        blend.blend_enable = vk::TRUE;
        blend.src_color_blend_factor = vk::BlendFactor::SRC_ALPHA;
        blend.dst_color_blend_factor = vk::BlendFactor::ONE_MINUS_SRC_ALPHA;
        blend.color_blend_op = vk::BlendOp::ADD;
        blend.src_alpha_blend_factor = vk::BlendFactor::ONE;
        blend.dst_alpha_blend_factor = vk::BlendFactor::ZERO;
        blend.alpha_blend_op = vk::BlendOp::ADD;
        self
    }

    pub fn enable_additive_blending(mut self) -> Self {
        let blend = &mut self.color_blend_attachment;
        blend.color_write_mask = vk::ColorComponentFlags::RGBA;
        blend.blend_enable = vk::TRUE;
        blend.src_color_blend_factor = vk::BlendFactor::ONE;
        blend.dst_color_blend_factor = vk::BlendFactor::DST_ALPHA;
        blend.color_blend_op = vk::BlendOp::ADD;
        blend.src_alpha_blend_factor = vk::BlendFactor::ONE;
        blend.dst_alpha_blend_factor = vk::BlendFactor::ZERO;
        blend.alpha_blend_op = vk::BlendOp::ADD;
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

    pub fn depth_test_enable(
        mut self,
        enable: bool,
        compare: Option<vk::CompareOp>,
    ) -> Self {
        self.depth_stencil.depth_test_enable =
            if enable { vk::TRUE } else { vk::FALSE };
        self.depth_stencil.depth_write_enable =
            if enable { vk::TRUE } else { vk::FALSE };
        self.depth_stencil.depth_compare_op = if enable {
            if let Some(compare) = compare {
                compare
            } else {
                vk::CompareOp::LESS_OR_EQUAL
            }
        } else {
            vk::CompareOp::ALWAYS
        };
        self.depth_stencil.min_depth_bounds = 0.0;
        self.depth_stencil.max_depth_bounds = 1.0;
        self
    }

    pub fn vertex_input(mut self, desc: VertexInputDescription) -> Self {
        self.vertex_input_desc = desc;
        self.vertex_input = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&self.vertex_input_desc.attributes)
            .vertex_binding_descriptions(&self.vertex_input_desc.bindings)
            .flags(self.vertex_input_desc.flags)
            .build();
        self
    }

    pub fn desc_sets(mut self, desc_sets: Vec<vk::DescriptorSet>) -> Self {
        self.desc_sets = desc_sets;
        self
    }

    pub fn build(mut self) -> Result<Material> {
        let device = self.device;

        let shader = self
            .shader
            .take()
            .ok_or_eyre("No shader provided for GraphicsMaterialBuilder")?;
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

        let pipeline_layout = self.pipeline_layout.take().ok_or_eyre(
            "No pipeline layout provided for GraphicsMaterialBuilder",
        )?;

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

        let pipeline = unsafe {
            match device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[pipeline_info],
                None,
            ) {
                Ok(pipelines) => Ok(pipelines),
                Err(_) => Err(eyre!("Failed to create graphic pipelines")),
            }
        }?[0];
        shader.cleanup(device);

        Ok(Material {
            pipeline,
            pipeline_layout,
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
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
        // Enable alpha blending by default
        vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
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

impl<'a> Drop for GraphicsMaterialBuilder<'a> {
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

pub struct ComputeMaterialBuilder<'a> {
    device: &'a ash::Device,
    shader: Option<ComputeShader>,
    pipeline_layout: Option<vk::PipelineLayout>,
}

impl<'a> ComputeMaterialBuilder<'a> {
    pub fn new(device: &'a ash::Device) -> Self {
        Self {
            device,
            shader: None,
            pipeline_layout: None,
        }
    }

    pub fn shader(mut self, shader: ComputeShader) -> Self {
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

    pub fn build(mut self) -> Result<Material> {
        let shader = self
            .shader
            .take()
            .ok_or_eyre("No shader provided for ComputeMaterialBuilder")?;
        let pipeline_layout = self.pipeline_layout.take().ok_or_eyre(
            "No pipeline layout provided for ComputeMaterialBuilder",
        )?;

        let name = CString::new("main")?;
        let stage_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader.shader_mod)
            .name(&name)
            .build();

        let pipeline_info = vk::ComputePipelineCreateInfo::builder()
            .layout(pipeline_layout)
            .stage(stage_info)
            .build();
        let pipeline = unsafe {
            match self.device.create_compute_pipelines(
                vk::PipelineCache::null(),
                &[pipeline_info],
                None,
            ) {
                Ok(pipelines) => Ok(pipelines),
                Err(_) => Err(eyre!("Failed to create compute pipeline")),
            }
        }?[0];
        shader.cleanup(self.device);

        Ok(Material {
            pipeline,
            pipeline_layout,
            pipeline_bind_point: vk::PipelineBindPoint::COMPUTE,
        })
    }
}

impl<'a> Drop for ComputeMaterialBuilder<'a> {
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

/// To be written into uniform buffers
struct MaterialConstants {
    color_factors: Vec4,
    metal_rough_factors: Vec4,
    padding: [Vec4; 14], // Padding to 256 bytes
}

struct MaterialResources {
    color_image: AllocatedImage,
    color_sampler: vk::Sampler,
    metal_rough_image: AllocatedImage,
    metal_rough_sampler: vk::Sampler,
    data_buffer: vk::Buffer,
    data_buffer_offset: u32,
}

struct GltfMetallicRoughness {
    opaque_material: Material,
    transparent_material: Material,
    material_layout: vk::DescriptorSetLayout,
    writer: DescriptorWriter,
}

impl GltfMetallicRoughness {
    pub fn new(
        ctx: &Context,
        swapchain: &Swapchain,
        scene_layout: vk::DescriptorSetLayout,
    ) -> Result<Self> {
        let shader = GraphicsShader::new("mesh.glsl", &ctx.device)?;

        let matrix_range = [vk::PushConstantRange {
            offset: 0,
            size: std::mem::size_of::<GpuDrawPushConstants>() as u32,
            stage_flags: vk::ShaderStageFlags::VERTEX,
        }];

        let material_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::UNIFORM_BUFFER,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .add_binding(
                1,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .add_binding(
                2,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .build(&ctx.device)?;

        let set_layouts = [scene_layout, material_layout];
        let mesh_pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&matrix_range)
            .build();
        let mesh_pipeline_layout = unsafe {
            ctx.device
                .create_pipeline_layout(&mesh_pipeline_layout_info, None)?
        };

        let opaque_material = Material::builder_graphics(&ctx.device)
            .shader(shader.clone())
            .input_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK, vk::FrontFace::CLOCKWISE)
            .disable_multisampling()
            .disable_blending()
            .depth_test_enable(true, Some(vk::CompareOp::GREATER_OR_EQUAL))
            .color_attachment_format(swapchain.image_format)
            .depth_attachment_format(swapchain.depth_image.format)
            .pipeline_layout(mesh_pipeline_layout)
            .build()?;

        let transparent_material = Material::builder_graphics(&ctx.device)
            .shader(shader)
            .input_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK, vk::FrontFace::CLOCKWISE)
            .disable_multisampling()
            .enable_additive_blending()
            .depth_test_enable(false, None)
            .color_attachment_format(swapchain.image_format)
            .depth_attachment_format(swapchain.depth_image.format)
            .pipeline_layout(mesh_pipeline_layout)
            .build()?;

        Ok(Self {
            opaque_material,
            transparent_material,
            material_layout,
            writer: DescriptorWriter::new(),
        })
    }
    fn clear_resources(ctx: &Context) {}
}

use std::ffi::{c_void, CStr};

use ash::vk;

// Info about a single shader stage for pipeline
pub fn pipeline_shader_stage_create_info(
    stage: vk::ShaderStageFlags,
    shader_module: vk::ShaderModule,
) -> vk::PipelineShaderStageCreateInfo {
    vk::PipelineShaderStageCreateInfo {
        stage,
        module: shader_module,
        p_name: "main",
        ..Default::default()
    }
}

// Info for vertex buffers and vertex formats (equivalent to VAO)
pub fn vertex_input_state_create_info() -> vk::PipelineVertexInputStateCreateInfo {
    vk::PipelineVertexInputStateCreateInfo {
        vertex_binding_description_count: 0,
        vertex_attribute_description_count: 0,
        ..Default::default()
    }
}

// Info for what topology to draw like triangles, lines, points
pub fn input_assembly_create_info(
    topology: vk::PrimitiveTopology
) -> vk::PipelineInputAssemblyStateCreateInfo {
    vk::PipelineInputAssemblyStateCreateInfo {
        topology,
        primitive_restart_enable: vk::FALSE,
        ..Default::default()
    }
}

// Config for fixed-function rasterization
pub fn rasterization_state_create_info(
    polygon_mode: vk::PolygonMode
) -> vk::PipelineRasterizationStateCreateInfo {
    vk::PipelineRasterizationStateCreateInfo {
        depth_clamp_enable: vk::FALSE,
        // Discards all primitives before rasterization stage if true
        rasterizer_discard_enable: vk::FALSE,
        polygon_mode,
        line_width: 1.0,
        // Backface culling
        cull_mode: vk::CullModeFlags::NONE,
        front_face: vk::FrontFace::CLOCKWISE,
        // No depth bias
        depth_bias_enable: vk::FALSE,
        depth_bias_constant_factor: 0.0,
        depth_bias_clamp: 0.0,
        depth_bias_slope_factor: 0.0,
        ..Default::default()
    }
}

// Config for MSAA for the pipeline
pub fn multisampling_state_create_info() -> vk::PipelineMultisampleStateCreateInfo {
    vk::PipelineMultisampleStateCreateInfo {
        sample_shading_enable: vk::FALSE,
        // Default to no multisampling (1 sample per pixel)
        rasterization_samples: vk::SampleCountFlags::TYPE_1,
        min_sample_shading: 1.0,
        p_sample_mask: std::ptr::null(),
        alpha_to_coverage_enable: vk::FALSE,
        alpha_to_one_enable: vk::FALSE,
        ..Default::default()
    }
}

pub fn color_blend_attachment_state() -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::RGBA,
        blend_enable: vk::FALSE,
        ..Default::default()
    }
}

pub fn pipeline_layout_create_info() -> vk::PipelineLayoutCreateInfo {
    vk::PipelineLayoutCreateInfo {
        flags: vk::PipelineLayoutCreateFlags::empty(),
        set_layout_count: 0,
        p_set_layouts: vk::DescriptorSetLayout::null(),
        push_constant_range_count: 0,
        p_push_constant_ranges: std::ptr::null(),
        ..Default::default()
    }
}

pub fn debug_utils_messenger_create_info(
) -> vk::DebugUtilsMessengerCreateInfoEXT {
    let message_severity =
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
    let message_type =
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE;
    vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(message_severity)
        .message_type(message_type)
        .pfn_user_callback(Some(debug_callback))
        .build()
}

unsafe extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let msg_severity = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => "[Verbose]",
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "[Warning]",
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "[Error]",
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "[Info]",
        _ => "[Unknown]",
    };
    let msg_type = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
        _ => "[Unknown]",
    };
    let msg = CStr::from_ptr((*p_callback_data).p_message);
    log::debug!("{}{} {:?}", msg_severity, msg_type, msg);

    vk::FALSE
}

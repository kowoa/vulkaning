use bevy::log;
use std::ffi::{c_void, CStr, CString};

use ash::vk;

// Info about a single shader stage for pipeline
pub fn pipeline_shader_stage_create_info(
    stage: vk::ShaderStageFlags,
    shader_module: vk::ShaderModule,
    main_fn_name: &CString,
) -> vk::PipelineShaderStageCreateInfo {
    vk::PipelineShaderStageCreateInfo {
        flags: vk::PipelineShaderStageCreateFlags::empty(),
        stage,
        module: shader_module,
        p_name: main_fn_name.as_ptr(),
        ..Default::default()
    }
}

// Info for vertex buffers and vertex formats (equivalent to VAO)
pub fn vertex_input_state_create_info() -> vk::PipelineVertexInputStateCreateInfo
{
    vk::PipelineVertexInputStateCreateInfo {
        vertex_binding_description_count: 0,
        vertex_attribute_description_count: 0,
        ..Default::default()
    }
}

// Info for what topology to draw like triangles, lines, points
pub fn input_assembly_create_info(
    topology: vk::PrimitiveTopology,
) -> vk::PipelineInputAssemblyStateCreateInfo {
    vk::PipelineInputAssemblyStateCreateInfo {
        topology,
        primitive_restart_enable: vk::FALSE,
        ..Default::default()
    }
}

// Config for fixed-function rasterization
pub fn rasterization_state_create_info(
    polygon_mode: vk::PolygonMode,
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
pub fn multisampling_state_create_info(
) -> vk::PipelineMultisampleStateCreateInfo {
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
        p_set_layouts: std::ptr::null(),
        push_constant_range_count: 0,
        p_push_constant_ranges: std::ptr::null(),
        ..Default::default()
    }
}

pub fn image_create_info(
    format: vk::Format,
    usage_flags: vk::ImageUsageFlags,
    extent: vk::Extent3D,
) -> vk::ImageCreateInfo {
    let info = vk::ImageCreateInfo {
        image_type: vk::ImageType::TYPE_2D,
        format,
        extent,
        mip_levels: 1,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::OPTIMAL,
        usage: usage_flags,
        ..Default::default()
    };
    info
}

pub fn image_view_create_info(
    format: vk::Format,
    image: vk::Image,
    aspect_flags: vk::ImageAspectFlags,
) -> vk::ImageViewCreateInfo {
    vk::ImageViewCreateInfo {
        view_type: vk::ImageViewType::TYPE_2D,
        image,
        format,
        subresource_range: vk::ImageSubresourceRange {
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
            aspect_mask: aspect_flags,
        },
        ..Default::default()
    }
}

pub fn depth_stencil_create_info(
    depth_test_enable: bool,
    depth_write_enable: bool,
    depth_compare_op: vk::CompareOp,
) -> vk::PipelineDepthStencilStateCreateInfo {
    vk::PipelineDepthStencilStateCreateInfo {
        depth_test_enable: if depth_test_enable {
            vk::TRUE
        } else {
            vk::FALSE
        },
        depth_write_enable: if depth_write_enable {
            vk::TRUE
        } else {
            vk::FALSE
        },
        depth_compare_op: if depth_test_enable {
            depth_compare_op
        } else {
            vk::CompareOp::ALWAYS
        },
        depth_bounds_test_enable: vk::FALSE,
        min_depth_bounds: 0.0,
        max_depth_bounds: 1.0,
        stencil_test_enable: vk::FALSE,
        ..Default::default()
    }
}

pub fn descriptor_set_layout_binding(
    descriptor_type: vk::DescriptorType,
    stage_flags: vk::ShaderStageFlags,
    binding: u32,
) -> vk::DescriptorSetLayoutBinding {
    vk::DescriptorSetLayoutBinding {
        binding,
        descriptor_count: 1,
        descriptor_type,
        p_immutable_samplers: std::ptr::null(),
        stage_flags,
    }
}

pub fn write_descriptor_buffer(
    desc_type: vk::DescriptorType,
    dst_set: vk::DescriptorSet,
    dst_binding: u32,
    p_buffer_info: *const vk::DescriptorBufferInfo,
) -> vk::WriteDescriptorSet {
    vk::WriteDescriptorSet {
        dst_binding,
        dst_set,
        descriptor_count: 1,
        descriptor_type: desc_type,
        p_buffer_info,
        ..Default::default()
    }
}

pub fn command_buffer_begin_info(
    flags: vk::CommandBufferUsageFlags,
) -> vk::CommandBufferBeginInfo {
    vk::CommandBufferBeginInfo {
        flags,
        ..Default::default()
    }
}

pub fn submit_info(cmd: &vk::CommandBuffer) -> vk::SubmitInfo {
    vk::SubmitInfo {
        wait_semaphore_count: 0,
        p_wait_semaphores: std::ptr::null(),
        p_wait_dst_stage_mask: std::ptr::null(),
        command_buffer_count: 1,
        p_command_buffers: cmd, // Make sure cmd lives longer than the returned vk::SubmitInfo
        signal_semaphore_count: 0,
        p_signal_semaphores: std::ptr::null(),
        ..Default::default()
    }
}

pub fn sampler_create_info(
    filter: vk::Filter,
    sampler_address_mode: vk::SamplerAddressMode,
) -> vk::SamplerCreateInfo {
    vk::SamplerCreateInfo {
        mag_filter: filter,
        min_filter: filter,
        address_mode_u: sampler_address_mode,
        address_mode_v: sampler_address_mode,
        address_mode_w: sampler_address_mode,
        ..Default::default()
    }
}

pub fn write_descriptor_image(
    desc_type: vk::DescriptorType,
    dst_set: vk::DescriptorSet,
    dst_binding: u32,
    p_image_info: *const vk::DescriptorImageInfo,
) -> vk::WriteDescriptorSet {
    vk::WriteDescriptorSet {
        dst_binding,
        dst_set,
        descriptor_count: 1,
        descriptor_type: desc_type,
        p_image_info,
        ..Default::default()
    }
}

pub fn image_subresource_range(
    aspect_mask: vk::ImageAspectFlags,
) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    }
}

pub fn attachment_info(
    view: vk::ImageView,
    clear: Option<vk::ClearValue>,
    layout: vk::ImageLayout,
) -> vk::RenderingAttachmentInfo {
    vk::RenderingAttachmentInfo::builder()
        .image_view(view)
        .image_layout(layout)
        .load_op(if clear.is_some() {
            vk::AttachmentLoadOp::CLEAR
        } else {
            vk::AttachmentLoadOp::LOAD
        })
        .store_op(vk::AttachmentStoreOp::STORE)
        .clear_value(if let Some(clear) = clear {
            clear
        } else {
            vk::ClearValue::default()
        })
        .build()
}

pub fn debug_utils_messenger_create_info(
) -> vk::DebugUtilsMessengerCreateInfoEXT {
    let message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
    let message_type = vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
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

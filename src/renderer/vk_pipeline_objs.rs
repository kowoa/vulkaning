use ash::vk;

pub struct VkPipelineObjs {}

struct PipelineBuilder {
    shader_stages: Vec<vk::PipelineShaderStageCreateInfo>,
    vertex_input_info: vk::PipelineVertexInputStateCreateInfo,
    input_assembly: vk::PipelineInputAssemblyStateCreateInfo,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
    rasterizer: vk::PipelineRasterizationStateCreateInfo,
    color_blend_attachment: vk::PipelineColorBlendAttachmentState,
    multisampling: vk::PipelineMultisampleStateCreateInfo,
    pipeline_layout: vk::PipelineLayout,
}



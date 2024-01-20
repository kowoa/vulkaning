// Asset initialization
// VkPipeline, VkBuffer, VkImage, VkRenderPass
pub mod mesh;
pub mod model;
pub mod pipeline;
pub mod renderpass;
pub mod shader;
pub mod vertex;

use ash::vk;
use gpu_allocator::vulkan::Allocator;
use mesh::Mesh;
use pipeline::PipelineBuilder;
use renderpass::Renderpass;
use shader::Shader;

use self::{
    mesh::MeshPushConstants, model::Model, pipeline::Pipeline, vertex::Vertex,
};

use super::{core::Core, swapchain::Swapchain, vk_initializers};

pub struct Assets {
    pub renderpasses: Vec<Renderpass>,
    pub pipelines: Vec<Pipeline>,
    pub models: Vec<Model>,
}

impl Assets {
    pub fn new(
        core: &mut Core,
        swapchain: &Swapchain,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> {
        let renderpass = Renderpass::new(&core.device, swapchain, window)?;
        let pipeline = create_pipeline(core, swapchain, &renderpass)?;
        let model = Model::load_from_obj("assets/monkey_smooth.obj", core)?;

        Ok(Self {
            renderpasses: vec![renderpass],
            pipelines: vec![pipeline],
            models: vec![model],
        })
    }

    pub fn cleanup(
        self,
        device: &ash::Device,
        allocator: &mut Allocator
    ) {
        log::info!("Cleaning up assets ...");

        for model in self.models {
            model.cleanup(device, allocator);
        }

        for pipeline in self.pipelines {
            pipeline.cleanup(device);
        }

        for renderpass in self.renderpasses {
            renderpass.cleanup(device);
        }
    }
}

fn create_pipeline(
    core: &mut Core,
    swapchain: &Swapchain,
    renderpass: &Renderpass,
) -> anyhow::Result<Pipeline> {
    let mut layout_info = vk_initializers::pipeline_layout_create_info();

    let push_constant = vk::PushConstantRange {
        offset: 0,
        size: std::mem::size_of::<MeshPushConstants>() as u32,
        stage_flags: vk::ShaderStageFlags::VERTEX,
    };

    layout_info.p_push_constant_ranges = &push_constant;
    layout_info.push_constant_range_count = 1;

    let layout =
        unsafe { core.device.create_pipeline_layout(&layout_info, None)? };

    let shader = Shader::new("tri-mesh", &core.device)?;

    let pipeline = PipelineBuilder::new(
        &shader.vert_shader_mod,
        &shader.frag_shader_mod,
        &core.device,
        swapchain,
    )?
    .pipeline_layout(layout, &core.device)
    .vertex_input(Vertex::get_vertex_desc())
    .build(&core.device, renderpass.renderpass)?;

    shader.destroy(&core.device);

    Ok(pipeline)
}

fn create_mesh(core: &mut Core) -> anyhow::Result<Mesh> {
    let vertices = vec![
        Vertex {
            position: [-0.5, -0.5, 0.0].into(),
            normal: [0.0, 0.0, 1.0].into(),
            color: [1.0, 0.0, 0.0].into(),
        },
        Vertex {
            position: [0.5, -0.5, 0.0].into(),
            normal: [0.0, 0.0, 1.0].into(),
            color: [0.0, 1.0, 0.0].into(),
        },
        Vertex {
            position: [0.0, 0.5, 0.0].into(),
            normal: [0.0, 0.0, 1.0].into(),
            color: [0.0, 0.0, 1.0].into(),
        },
    ];

    let mesh = Mesh::new(vertices, &core.device, &mut core.allocator)?;

    Ok(mesh)
}

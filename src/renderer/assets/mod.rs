// Asset initialization
// VkPipeline, VkBuffer, VkImage, VkRenderPass
mod pipeline;
mod renderpass;
mod shader;
mod mesh;

use pipeline::PipelineBuilder;
use renderpass::Renderpass;
use shader::Shader;
use mesh::Mesh;

use self::pipeline::Pipeline;

use super::{swapchain::Swapchain, core::Core};

pub struct Assets {
    pub renderpasses: Vec<Renderpass>,
    pub pipelines: Vec<Pipeline>,
    pub meshes: Vec<Mesh>,
}

impl Assets {
    pub fn new(
        core: &Core,
        swapchain: &Swapchain,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> {
        let device = &core.device;
        
        let renderpass = Renderpass::new(device, swapchain, window)?;

        let shader_colored = Shader::new("colored-triangle", device)?;
        let shader = Shader::new("triangle", device)?;

        let pipeline_colored = PipelineBuilder::new(
            &shader_colored.vert_shader_mod,
            &shader_colored.frag_shader_mod,
            device,
            swapchain,
        )?
        .build(device, renderpass.renderpass)?;

        let pipeline = PipelineBuilder::new(
            &shader.vert_shader_mod,
            &shader.frag_shader_mod,
            device,
            swapchain,
        )?
        .build(device, renderpass.renderpass)?;

        shader_colored.destroy(device);
        shader.destroy(device);

        Ok(Self {
            renderpasses: vec![renderpass],
            pipelines: vec![pipeline_colored, pipeline],
            meshes: vec![],
        })
    }

    pub fn add_pipeline(&mut self, pipeline: Pipeline) {
        self.pipelines.push(pipeline);
    }

    pub fn destroy(&self, device: &ash::Device) {
        log::info!("Cleaning up assets ...");
        
        for pipeline in &self.pipelines {
            pipeline.destroy(device);
        }

        for renderpass in &self.renderpasses {
            renderpass.destroy(device);
        }
    }
}
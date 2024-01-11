// Asset initialization
// VkPipeline, VkBuffer, VkImage, VkRenderPass
mod pipeline;
mod renderpass;
mod shader;
mod mesh;

use ash::vk::DeviceMemory;
use gpu_alloc::{UsageFlags, MemoryBlock, Request, GpuAllocator};
use gpu_alloc_ash::AshMemoryDevice;
use pipeline::PipelineBuilder;
use renderpass::Renderpass;
use shader::Shader;
use mesh::Mesh;

use self::{pipeline::Pipeline, mesh::Vertex};

use super::{swapchain::Swapchain, core::Core};

pub struct Assets {
    pub renderpasses: Vec<Renderpass>,
    pub pipelines: Vec<Pipeline>,
    pub meshes: Vec<Mesh>,
}

impl Assets {
    pub fn new(
        core: &mut Core,
        swapchain: &Swapchain,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> {
        let mesh = create_mesh(core)?;
        let device = &core.device;
        
        let renderpass = Renderpass::new(device, swapchain, window)?;

        //let shader_colored = Shader::new("colored-triangle", device)?;
        //let shader_red = Shader::new("red-triangle", device)?;
        let shader_tri_mesh = Shader::new("tri-mesh", device)?;

        /*
        let pipeline_colored = PipelineBuilder::new(
            &shader_colored.vert_shader_mod,
            &shader_colored.frag_shader_mod,
            device,
            swapchain,
        )?
        .build(device, renderpass.renderpass)?;

        let pipeline_red = PipelineBuilder::new(
            &shader_red.vert_shader_mod,
            &shader_red.frag_shader_mod,
            device,
            swapchain,
        )?
        .build(device, renderpass.renderpass)?;
        */

        let pipeline_tri_mesh = PipelineBuilder::new(
            &shader_tri_mesh.vert_shader_mod,
            &shader_tri_mesh.frag_shader_mod,
            device,
            swapchain,
        )?
        .vertex_input(Vertex::get_vertex_desc())
        .build(device, renderpass.renderpass)?;


        //shader_colored.destroy(device);
        //shader_red.destroy(device);
        shader_tri_mesh.destroy(device);

        Ok(Self {
            renderpasses: vec![renderpass],
            //pipelines: vec![pipeline_tri_mesh, pipeline_colored, pipeline_red],
            pipelines: vec![pipeline_tri_mesh],
            meshes: vec![mesh],
        })
    }

    pub fn add_pipeline(&mut self, pipeline: Pipeline) {
        self.pipelines.push(pipeline);
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut GpuAllocator<DeviceMemory>) {
        log::info!("Cleaning up assets ...");

        for mesh in self.meshes {
            mesh.cleanup(device, allocator);
        }
        
        for pipeline in self.pipelines {
            pipeline.cleanup(device);
        }

        for renderpass in self.renderpasses {
            renderpass.cleanup(device);
        }
    }
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
    
    let mesh = Mesh::new(vertices, core)?;

    Ok(mesh)
}
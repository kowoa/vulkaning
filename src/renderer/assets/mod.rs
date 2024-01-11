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

        let meshes = create_meshes(core)?;

        Ok(Self {
            renderpasses: vec![renderpass],
            pipelines: vec![pipeline_colored, pipeline],
            meshes,
        })
    }

    pub fn add_pipeline(&mut self, pipeline: Pipeline) {
        self.pipelines.push(pipeline);
    }

    pub fn destroy(self, device: &ash::Device, allocator: &mut GpuAllocator<DeviceMemory>) {
        log::info!("Cleaning up assets ...");

        for mesh in self.meshes {
            mesh.destroy(device, allocator);
        }
        
        for pipeline in &self.pipelines {
            pipeline.destroy(device);
        }

        for renderpass in &self.renderpasses {
            renderpass.destroy(device);
        }
    }
}

fn create_meshes(core: &mut Core) -> anyhow::Result<Vec<Mesh>> {
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

    let mut mem_block = unsafe {
        core.allocator.alloc(
            AshMemoryDevice::wrap(&core.device),
            Request {
                size: vertices.len() as u64 * std::mem::size_of::<Vertex>() as u64,
                align_mask: 0,
                usage: UsageFlags::UPLOAD,
                memory_types: !0,
            }
        )?
    };

    unsafe {
        mem_block.write_bytes(
            AshMemoryDevice::wrap(&core.device),
            0,
            bytemuck::cast_slice(&vertices),
        )?;
    }
    
    /*
    unsafe {
        core.allocator.dealloc(
            AshMemoryDevice::wrap(&core.device),
            mem_block,
        );
    }
    */
    
    let mesh = Mesh {
        vertices,
        mem_block,
    };

    Ok(vec![mesh])
}
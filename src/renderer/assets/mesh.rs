use ash::vk::{self, DeviceMemory};
use bytemuck::{offset_of, Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use gpu_alloc::{GpuAllocator, MemoryBlock, Request, UsageFlags};
use gpu_alloc_ash::AshMemoryDevice;

use crate::renderer::core::Core;

#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct MeshPushConstants {
    pub data: Vec4,
    pub render_matrix: Mat4,
}

#[derive(Debug)]
pub struct VertexInputDescription {
    pub bindings: Vec<vk::VertexInputBindingDescription>,
    pub attributes: Vec<vk::VertexInputAttributeDescription>,
    pub flags: vk::PipelineVertexInputStateCreateFlags,
}

#[derive(Copy, Clone, Pod, Zeroable, Default)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
}

impl Vertex {
    pub fn get_vertex_desc() -> VertexInputDescription {
        let bindings = vec![vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];

        let attributes = vec![
            // Position
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Vertex, position) as u32,
            },
            // Normal
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Vertex, normal) as u32,
            },
            // Color
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 2,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Vertex, color) as u32,
            },
        ];

        VertexInputDescription {
            bindings,
            attributes,
            flags: vk::PipelineVertexInputStateCreateFlags::empty(),
            //flags: Default::default(),
        }
    }
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub vertex_buffer: vk::Buffer,
    pub mem_block: MemoryBlock<DeviceMemory>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, core: &mut Core) -> anyhow::Result<Self> {
        let device = AshMemoryDevice::wrap(&core.device);

        let vertex_buffer = {
            let buffer_info = vk::BufferCreateInfo {
                size: (vertices.len() * std::mem::size_of::<Vertex>()) as u64,
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                ..Default::default()
            };

            let buffer =
                unsafe { core.device.create_buffer(&buffer_info, None)? };

            buffer
        };

        let reqs = unsafe {
            core.device.get_buffer_memory_requirements(vertex_buffer)
        };

        let mut mem_block = unsafe {
            core.allocator.alloc(
                device,
                Request {
                    size: reqs.size,
                    align_mask: reqs.alignment - 1,
                    usage: UsageFlags::UPLOAD,
                    memory_types: reqs.memory_type_bits,
                },
            )?
        };

        unsafe {
            core.device.bind_buffer_memory(
                vertex_buffer,
                *mem_block.memory(),
                mem_block.offset(),
            )?;
        }

        unsafe {
            mem_block.write_bytes(
                device,
                0,
                bytemuck::cast_slice(&vertices),
            )?;
        }

        Ok(Self {
            vertices,
            vertex_buffer,
            mem_block,
        })
    }

    pub fn cleanup(
        self,
        device: &ash::Device,
        allocator: &mut GpuAllocator<DeviceMemory>,
    ) {
        log::info!("Cleaning up mesh ...");
        unsafe {
            allocator.dealloc(AshMemoryDevice::wrap(device), self.mem_block);
            device.destroy_buffer(self.vertex_buffer, None);
        }
    }
}

use ash::vk;
use bytemuck::{offset_of, Pod, Zeroable};
use glam::{Vec2, Vec3};

#[derive(Debug)]
pub struct VertexInputDescription {
    pub bindings: Vec<vk::VertexInputBindingDescription>,
    pub attributes: Vec<vk::VertexInputAttributeDescription>,
    pub flags: vk::PipelineVertexInputStateCreateFlags,
}

impl Default for VertexInputDescription {
    fn default() -> Self {
        Vertex::get_vertex_desc()
    }
}

#[derive(Copy, Clone, Pod, Zeroable, Default, Debug)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
    pub texcoord: Vec2,
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
            // Texcoord (UV)
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 3,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Vertex, texcoord) as u32,
            },
        ];

        VertexInputDescription {
            bindings,
            attributes,
            flags: vk::PipelineVertexInputStateCreateFlags::empty(),
        }
    }
}

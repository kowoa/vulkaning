use crate::renderer::assets::vertex::Vertex;
use ash::vk;
use ash::vk::DeviceMemory;
use gpu_alloc::{GpuAllocator, MemoryBlock, Request, UsageFlags};
use gpu_alloc_ash::AshMemoryDevice;

pub struct AllocatedBuffer {
    pub buffer: vk::Buffer,
    pub allocation: MemoryBlock<DeviceMemory>,
}

impl AllocatedBuffer {
    pub fn new(
        vertices: &Vec<Vertex>,
        device: &ash::Device,
        allocator: &mut GpuAllocator<DeviceMemory>,
    ) -> anyhow::Result<Self> {
        let vertex_buffer = {
            let buffer_info = vk::BufferCreateInfo {
                size: (vertices.len() * std::mem::size_of::<Vertex>()) as u64,
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                ..Default::default()
            };

            let buffer = unsafe { device.create_buffer(&buffer_info, None)? };

            buffer
        };

        let reqs =
            unsafe { device.get_buffer_memory_requirements(vertex_buffer) };

        let mut mem_block = unsafe {
            allocator.alloc(
                AshMemoryDevice::wrap(device),
                Request {
                    size: reqs.size,
                    align_mask: reqs.alignment - 1,
                    usage: UsageFlags::UPLOAD,
                    memory_types: reqs.memory_type_bits,
                },
            )?
        };

        unsafe {
            device.bind_buffer_memory(
                vertex_buffer,
                *mem_block.memory(),
                mem_block.offset(),
            )?;
        }

        unsafe {
            mem_block.write_bytes(
                AshMemoryDevice::wrap(device),
                0,
                bytemuck::cast_slice(&vertices),
            )?;
        }

        Ok(Self {
            buffer: vertex_buffer,
            allocation: mem_block,
        })
    }

    pub fn cleanup(
        self,
        device: &ash::Device,
        allocator: &mut GpuAllocator<DeviceMemory>,
    ) {
        unsafe {
            allocator.dealloc(AshMemoryDevice::wrap(device), self.allocation);
            device.destroy_buffer(self.buffer, None);
        }
    }
}

pub struct AllocatedImage {
    pub image: vk::Image,
    pub allocation: MemoryBlock<DeviceMemory>,
}

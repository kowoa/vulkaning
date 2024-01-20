use crate::renderer::assets::vertex::Vertex;
use ash::vk;
use gpu_allocator::{vulkan::{Allocation, Allocator, AllocationCreateDesc, AllocationScheme}, MemoryLocation};

pub struct AllocatedBuffer {
    pub buffer: vk::Buffer,
    pub allocation: Allocation,
}

impl AllocatedBuffer {
    pub fn new(
        vertices: &Vec<Vertex>,
        device: &ash::Device,
        allocator: &mut Allocator,
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

        let mut allocation = allocator
            .allocate(&AllocationCreateDesc {
                name: "Vertex Buffer Allocation",
                requirements: reqs,
                location: MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;

        unsafe {
            device.bind_buffer_memory(
                vertex_buffer,
                allocation.memory(),
                allocation.offset(),
            )?;
        }

        let _copy_record = presser::copy_from_slice_to_offset(
            &vertices[..],
            &mut allocation,
            0,
        )?;

        Ok(Self {
            buffer: vertex_buffer,
            allocation,
        })
    }

    pub fn cleanup(
        self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) {
        unsafe {
            allocator.free(self.allocation).unwrap();
            device.destroy_buffer(self.buffer, None);
        }
    }
}

pub struct AllocatedImage {
    pub image: vk::Image,
    pub allocation: Allocation,
}

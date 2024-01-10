use std::ffi::c_void;

use ash::vk;
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator},
    MemoryLocation,
};

use super::mesh::Vertex;

pub struct AllocatedBuffer {
    buffer: vk::Buffer,
    allocation: Allocation,
}

impl AllocatedBuffer {
    pub fn new(
        vertices: &Vec<Vertex>,
        allocator: &mut Allocator,
        device: &ash::Device,
    ) -> anyhow::Result<Self> {
        let buffer_info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BUFFER_CREATE_INFO,
            size: vertices.len() as u64 * std::mem::size_of::<Vertex>() as u64,
            usage: vk::BufferUsageFlags::VERTEX_BUFFER,
            ..Default::default()
        };

        let buffer = unsafe { device.create_buffer(&buffer_info, None)? };
        let requirements =
            unsafe { device.get_buffer_memory_requirements(buffer) };

        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "Vertex Buffer",
            requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        unsafe {
            device.bind_buffer_memory(
                buffer,
                allocation.memory(),
                allocation.offset(),
            )?;

            std::ptr::copy_nonoverlapping(
                vertices.as_ptr() as *const c_void,
                allocation.mapped_ptr().unwrap().as_ptr() as *mut c_void,
                vertices.len() * std::mem::size_of::<Vertex>(),
            );
        }

        Ok(Self { buffer, allocation })
    }

    pub fn destroy(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            device.free_memory(self.allocation.memory(), None);
            allocator.free(self.allocation).unwrap();
            device.destroy_buffer(self.buffer, None);
        }
    }
}

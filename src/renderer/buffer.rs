use ash::vk;
use color_eyre::eyre::{eyre, OptionExt, Result};
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator},
    MemoryLocation,
};

#[derive(Debug)]
pub struct AllocatedBuffer {
    pub buffer: vk::Buffer,
    pub allocation: Allocation,
    pub size: u64,
    offsets: Option<Vec<u32>>,
}

impl AllocatedBuffer {
    pub fn new(
        device: &ash::Device,
        allocator: &mut Allocator,
        buffer_size: u64,
        buffer_usage: vk::BufferUsageFlags,
        alloc_name: &str,
        alloc_loc: MemoryLocation,
    ) -> Result<Self> {
        let buffer = {
            let buffer_info = vk::BufferCreateInfo {
                size: buffer_size,
                usage: buffer_usage,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                ..Default::default()
            };
            unsafe { device.create_buffer(&buffer_info, None)? }
        };

        let reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: alloc_name,
            requirements: reqs,
            location: alloc_loc,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;

        unsafe {
            device.bind_buffer_memory(
                buffer,
                allocation.memory(),
                allocation.offset(),
            )?;
        }

        Ok(Self {
            buffer,
            allocation,
            size: buffer_size,
            offsets: None,
        })
    }

    pub fn set_offsets(&mut self, offsets: Vec<u32>) {
        self.offsets = Some(offsets);
    }

    pub fn get_offset(&self, index: u32) -> Result<u32> {
        Ok(*self
            .offsets
            .as_ref()
            .ok_or_eyre(eyre!("No offsets set"))?
            .get(index as usize)
            .ok_or_eyre(eyre!("Invalid offset index"))?)
    }

    pub fn write<T>(
        &mut self,
        data: &[T],
        start_offset: usize,
    ) -> Result<presser::CopyRecord>
    where
        T: Copy,
    {
        Ok(presser::copy_from_slice_to_offset(
            data,
            &mut self.allocation,
            start_offset,
        )?)
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            allocator.free(self.allocation).unwrap();
            device.destroy_buffer(self.buffer, None);
        }
    }
}

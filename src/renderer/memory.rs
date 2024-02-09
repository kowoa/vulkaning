use ash::vk;
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator},
    MemoryLocation,
};
use color_eyre::eyre::Result;

use super::{resources::vertex::Vertex, vk_initializers};

#[derive(Debug)]
pub struct AllocatedBuffer {
    pub buffer: vk::Buffer,
    pub allocation: Allocation,
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

        Ok(Self { buffer, allocation })
    }

    pub fn new_vertex_buffer(
        vertices: &Vec<Vertex>,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let mut buffer = Self::new(
            device,
            allocator,
            (vertices.len() * std::mem::size_of::<Vertex>()) as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "Vertex Buffer Allocation",
            MemoryLocation::CpuToGpu,
        )?;

        let _copy_record = buffer.write(&vertices[..])?;

        Ok(buffer)
    }

    pub fn write<T>(&mut self, data: &[T]) -> Result<presser::CopyRecord>
    where
        T: Copy,
    {
        Ok(presser::copy_from_slice_to_offset(
            data,
            &mut self.allocation,
            0,
        )?)
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            allocator.free(self.allocation).unwrap();
            device.destroy_buffer(self.buffer, None);
        }
    }
}

pub struct AllocatedImage {
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub image_format: vk::Format,
    pub allocation: Allocation,
}

impl AllocatedImage {
    pub fn new_depth_image(
        extent: vk::Extent3D,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<Self> {
        let format = vk::Format::D32_SFLOAT;

        let image = {
            let usage_flags = vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
            let info =
                vk_initializers::image_create_info(format, usage_flags, extent);
            unsafe { device.create_image(&info, None)? }
        };

        let reqs = unsafe { device.get_image_memory_requirements(image) };

        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "Depth Image Allocation",
            requirements: reqs,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::DedicatedImage(image),
        })?;

        unsafe {
            device.bind_image_memory(image, allocation.memory(), 0)?;
        }

        let image_view = {
            let info = vk_initializers::image_view_create_info(
                format,
                image,
                vk::ImageAspectFlags::DEPTH,
            );
            unsafe { device.create_image_view(&info, None)? }
        };

        Ok(Self {
            image,
            image_view,
            image_format: format,
            allocation,
        })
    }

    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            device.destroy_image_view(self.image_view, None);
            allocator.free(self.allocation).unwrap();
            device.destroy_image(self.image, None);
        }
    }
}

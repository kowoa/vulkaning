use crate::renderer::assets::vertex::Vertex;
use ash::vk;
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator},
    MemoryLocation,
};

use super::vk_initializers;

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

        let mut allocation = allocator.allocate(&AllocationCreateDesc {
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
    ) -> anyhow::Result<Self> {
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

use ash::vk;
use color_eyre::eyre::{eyre, OptionExt, Result};
use gpu_allocator::vulkan::Allocator;

use crate::renderer::buffer::AllocatedBuffer;

use super::{context::Context, gpu_data::GpuVertexData, mesh::Mesh};

#[derive(Debug)]
pub struct Model {
    meshes: Vec<Mesh>,
    vertex_buffer: Option<AllocatedBuffer>,
    index_buffer: Option<AllocatedBuffer>,
    vertex_buffer_address: Option<vk::DeviceAddress>,
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.meshes
            .iter()
            .zip(other.meshes.iter())
            .all(|(mesh, other)| mesh == other)
    }
}

impl Model {
    pub fn new(meshes: Vec<Mesh>) -> Self {
        Self {
            meshes,
            vertex_buffer: None,
            index_buffer: None,
            vertex_buffer_address: None,
        }
    }

    pub fn draw(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
    ) -> Result<()> {
        self.bind_vertex_buffer(cmd, device)?;
        self.bind_index_buffer(cmd, device)?;

        // Draw this render object's model
        let index_count = self.meshes.iter().map(|mesh| mesh.index_count).sum();
        unsafe {
            device.cmd_draw_indexed(cmd, index_count, 1, 0, 0, 0);
        }

        Ok(())
    }

    fn bind_vertex_buffer(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
    ) -> Result<()> {
        let buffer = self
            .vertex_buffer
            .as_ref()
            .ok_or_eyre("No vertex buffer found")?;
        unsafe {
            device.cmd_bind_vertex_buffers(cmd, 0, &[buffer.buffer], &[0]);
        }
        Ok(())
    }

    fn bind_index_buffer(
        &self,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
    ) -> Result<()> {
        let buffer = self
            .index_buffer
            .as_ref()
            .ok_or_eyre("No vertex buffer found")?;
        unsafe {
            device.cmd_bind_index_buffer(
                cmd,
                buffer.buffer,
                0,
                vk::IndexType::UINT32,
            );
        }
        Ok(())
    }

    pub fn upload(
        &mut self,
        ctx: &Context,
        allocator: &mut Allocator,
    ) -> Result<()> {
        self.upload_vertices(ctx, allocator)?;
        self.upload_indices(ctx, allocator)?;
        Ok(())
    }

    fn upload_vertices(
        &mut self,
        ctx: &Context,
        allocator: &mut Allocator,
    ) -> Result<()> {
        let mut vertices = Vec::new();
        for mesh in &mut self.meshes {
            let mesh_vertices = mesh
                .vertices
                .take()
                .ok_or_eyre("No vertices found in mesh")?;
            vertices.extend(mesh_vertices);
        }
        let vertices = vertices
            .iter()
            .map(|v| v.as_gpu_data())
            .collect::<Vec<GpuVertexData>>();

        let buffer_size =
            (vertices.len() * std::mem::size_of::<GpuVertexData>()) as u64;
        // Create CPU-side staging buffer
        let mut staging_buffer = AllocatedBuffer::new(
            &ctx.device,
            allocator,
            buffer_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            "Model staging buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        // Copy vertex data into staging buffer
        let _ = staging_buffer.write(&vertices[..], 0)?;

        // Create GPU-side vertex buffer if it doesn't already exist
        if self.vertex_buffer.is_none() {
            let buffer = AllocatedBuffer::new(
                &ctx.device,
                allocator,
                buffer_size,
                // Use this buffer to render meshes and copy data into
                vk::BufferUsageFlags::VERTEX_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                "Model vertex buffer",
                gpu_allocator::MemoryLocation::GpuOnly,
            )?;
            self.vertex_buffer_address = Some(unsafe {
                ctx.device.get_buffer_device_address(
                    &vk::BufferDeviceAddressInfo {
                        buffer: buffer.buffer,
                        ..Default::default()
                    },
                )
            });
            self.vertex_buffer = Some(buffer);
        }

        // Execute immediate command to transfer data from staging buffer to vertex buffer
        if let Some(vertex_buffer) = &self.vertex_buffer {
            ctx.execute_one_time_command(
                |cmd: vk::CommandBuffer, device: &ash::Device| {
                    let copy = vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: buffer_size,
                    };
                    unsafe {
                        device.cmd_copy_buffer(
                            cmd,
                            staging_buffer.buffer,
                            vertex_buffer.buffer,
                            &[copy],
                        );
                    }

                    Ok(())
                },
            )?;

            // At this point, the vertex buffer should be populated with data from the staging buffer
            // Destroy staging buffer now because the vertex buffer now holds the data
            staging_buffer.cleanup(&ctx.device, allocator);

            Ok(())
        } else {
            staging_buffer.cleanup(&ctx.device, allocator);
            Err(eyre!("Vertex buffer not created"))
        }
    }

    fn upload_indices(
        &mut self,
        ctx: &Context,
        allocator: &mut Allocator,
    ) -> Result<()> {
        let mut offset = 0;
        let mut indices = Vec::new();
        for mesh in &mut self.meshes {
            let mut mesh_indices =
                mesh.indices.take().ok_or_eyre("No indices found in mesh")?;
            let index_count = mesh_indices.len() as u32;
            mesh_indices.iter_mut().for_each(|i| *i += offset);
            indices.extend(mesh_indices);
            offset += index_count;
        }

        let buffer_size = (indices.len() * std::mem::size_of::<u32>()) as u64;
        // Create CPU-side staging buffer
        let mut staging_buffer = AllocatedBuffer::new(
            &ctx.device,
            allocator,
            buffer_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            "Model index staging buffer",
            gpu_allocator::MemoryLocation::CpuToGpu,
        )?;

        // Copy vertex data into staging buffer
        let _ = staging_buffer.write(&indices[..], 0)?;

        // Create GPU-side index buffer if it doesn't already exist
        if self.index_buffer.is_none() {
            self.index_buffer = Some(AllocatedBuffer::new(
                &ctx.device,
                allocator,
                buffer_size,
                // Use this buffer to render meshes and copy data into
                vk::BufferUsageFlags::INDEX_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
                "Model index buffer",
                gpu_allocator::MemoryLocation::GpuOnly,
            )?);
        }

        // Execute immediate command to transfer data from staging buffer to vertex buffer
        if let Some(index_buffer) = &self.index_buffer {
            ctx.execute_one_time_command(
                |cmd: vk::CommandBuffer, device: &ash::Device| {
                    let copy = vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: buffer_size,
                    };
                    unsafe {
                        device.cmd_copy_buffer(
                            cmd,
                            staging_buffer.buffer,
                            index_buffer.buffer,
                            &[copy],
                        );
                    }

                    Ok(())
                },
            )?;

            // At this point, the vertex buffer should be populated with data from the staging buffer
            // Destroy staging buffer now because the vertex buffer now holds the data
            staging_buffer.cleanup(&ctx.device, allocator);

            Ok(())
        } else {
            staging_buffer.cleanup(&ctx.device, allocator);
            Err(eyre!("Index buffer not created"))
        }
    }
    pub fn cleanup(self, device: &ash::Device, allocator: &mut Allocator) {
        if let Some(vertex_buffer) = self.vertex_buffer {
            vertex_buffer.cleanup(device, allocator);
        }
        if let Some(index_buffer) = self.index_buffer {
            index_buffer.cleanup(device, allocator);
        }
    }
}

use std::collections::HashMap;

use ash::vk;
use bevy_egui::{egui, EguiRenderOutput};
use color_eyre::eyre::{eyre, Result};
use glam::Vec2;
use gpu_allocator::{vulkan::Allocator, MemoryLocation};

use super::{
    buffer::AllocatedBuffer,
    descriptors::{DescriptorAllocator, DescriptorSetLayoutBuilder},
    image::{AllocatedImage, AllocatedImageCreateInfo},
    material::Material,
    shader::GraphicsShader,
    swapchain::Swapchain,
    texture::Texture,
    upload_context::UploadContext,
    vertex::VertexInputDescription,
};

pub struct EguiRenderer {
    desc_set: vk::DescriptorSet,
    desc_set_layout: vk::DescriptorSetLayout,
    material: Material,
    vertex_buffer: AllocatedBuffer,
    index_buffer: AllocatedBuffer,

    managed_textures: ManagedTextures,
    //user_textures: UserTextures,
}

impl EguiRenderer {
    pub fn new(
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
        draw_image: &AllocatedImage,
        swapchain: &Swapchain,
    ) -> Result<Self> {
        let desc_set_layout = DescriptorSetLayoutBuilder::new()
            .add_binding(
                0,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::FRAGMENT,
            )
            .build(device)?;
        desc_allocator.add_layout("egui texture", desc_set_layout);
        let desc_set = desc_allocator.allocate(device, "egui texture")?;
        let pipeline_layout = {
            let set_layouts = [desc_set_layout];
            let push_constant_ranges = [
                // screen_size is a Vec2
                vk::PushConstantRange {
                    offset: 0,
                    size: std::mem::size_of::<Vec2>() as u32,
                    stage_flags: vk::ShaderStageFlags::VERTEX,
                },
            ];
            let layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&set_layouts)
                .push_constant_ranges(&push_constant_ranges)
                .build();
            unsafe { device.create_pipeline_layout(&layout_info, None)? }
        };
        let shader = GraphicsShader::new("egui", device)?;
        let material = Material::builder_graphics(device)
            .pipeline_layout(pipeline_layout)
            .shader(shader)
            .vertex_input(Self::get_vertex_desc())
            .color_attachment_format(draw_image.format)
            .depth_attachment_format(swapchain.depth_image.format)
            .build()?;
        let vertex_buffer = AllocatedBuffer::new(
            device,
            allocator,
            Self::default_vertex_buffer_size(),
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "egui vertex buffer",
            MemoryLocation::CpuToGpu,
        )?;
        let index_buffer = AllocatedBuffer::new(
            device,
            allocator,
            Self::default_vertex_buffer_size(),
            vk::BufferUsageFlags::INDEX_BUFFER,
            "egui index buffer",
            MemoryLocation::CpuToGpu,
        )?;

        let managed_textures = ManagedTextures::new();
        //let user_textures = UserTextures::new();

        Ok(Self {
            desc_set,
            desc_set_layout,
            material,
            vertex_buffer,
            index_buffer,
            managed_textures,
            //user_textures,
        })
    }

    // Call this AFTER a renderpass has begun
    pub fn draw_egui(
        &mut self,
        width: u32,
        height: u32,
        egui_context: &mut egui::Context,
        egui_output: &EguiRenderOutput,
        cmd: vk::CommandBuffer,
        device: &ash::Device,
        upload_context: &UploadContext,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
    ) {
        self.managed_textures.update_textures(
            cmd,
            &egui_output.textures_delta,
            upload_context,
            device,
            allocator,
            desc_allocator,
        );
        // Bind pipeline
        self.material.bind_pipeline(cmd, device);
        unsafe {
            // Bind vertex buffer
            device.cmd_bind_vertex_buffers(
                cmd,
                0,
                &[self.vertex_buffer.buffer],
                &[0],
            );
            // Bind index buffer
            device.cmd_bind_index_buffer(
                cmd,
                self.index_buffer.buffer,
                0,
                vk::IndexType::UINT32,
            );
        }
        // Bind descriptor set
        self.material
            .bind_desc_sets(cmd, device, 0, &[self.desc_set], &[]);
        // Update push constants
        let screen_size = Vec2::new(
            width as f32 / egui_context.zoom_factor(),
            height as f32 / egui_context.zoom_factor(),
        );
        self.material.update_push_constants(
            cmd,
            device,
            vk::ShaderStageFlags::VERTEX,
            bytemuck::cast_slice(&[screen_size]),
        );

        let mut vertex_base = 0;
        let mut index_base = 0;

        let clipped_primitives = &egui_output.paint_jobs;
        let textures_delta = &egui_output.textures_delta;
        for egui::ClippedPrimitive {
            clip_rect,
            primitive,
        } in clipped_primitives
        {
            let mesh = match primitive {
                egui::epaint::Primitive::Mesh(mesh) => Ok(mesh),
                egui::epaint::Primitive::Callback(callback) => {
                    Err(eyre!("PaintCallback: {:#?}", callback))
                }
            }
            .unwrap();
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }

            // Bind texture
            match mesh.texture_id {
                egui::TextureId::Managed(_) => self.material.bind_desc_sets(
                    cmd,
                    device,
                    0,
                    &[self
                        .managed_textures
                        .textures
                        .get(&mesh.texture_id)
                        .unwrap()
                        .desc_set()],
                    &[],
                ),
                egui::TextureId::User(id) => {
                    panic!("Texture is User Managed");
                    /*
                                        if let Some(&desc_set) =
                                            self.user_textures.desc_sets.get(&id)
                                        {
                                            self.material.bind_desc_sets(
                                                cmd,
                                                device,
                                                0,
                                                &[desc_set],
                                                &[],
                                            );
                                        } else {
                                            log::error!(
                                                "UserTexture has already been unregistered: {:?}",
                                                mesh.texture_id
                                            );
                                            continue;
                                        }
                    */
                }
            }

            // Write to vertex and index buffers
            let _ = self.vertex_buffer.write(&mesh.vertices, 0);
            let _ = self.index_buffer.write(&mesh.indices, 0);

            // Update scissor and viewport
            let min = {
                let min = clip_rect.min;
                let min = egui::Pos2 {
                    x: min.x * egui_context.zoom_factor(),
                    y: min.y * egui_context.zoom_factor(),
                };
                let min = egui::Pos2 {
                    x: f32::clamp(min.x, 0.0, width as f32),
                    y: f32::clamp(min.y, 0.0, height as f32),
                };
                min
            };
            let max = {
                let max = clip_rect.max;
                let max = egui::Pos2 {
                    x: max.x * egui_context.zoom_factor(),
                    y: max.y * egui_context.zoom_factor(),
                };
                let max = egui::Pos2 {
                    x: f32::clamp(max.x, min.x, width as f32),
                    y: f32::clamp(max.y, min.y, height as f32),
                };
                max
            };
            unsafe {
                device.cmd_set_scissor(
                    cmd,
                    0,
                    std::slice::from_ref(
                        &vk::Rect2D::builder()
                            .offset(vk::Offset2D {
                                x: min.x.round() as i32,
                                y: min.y.round() as i32,
                            })
                            .extent(vk::Extent2D {
                                width: (max.x.round() - min.x) as u32,
                                height: (max.y.round() - min.y) as u32,
                            }),
                    ),
                );
                device.cmd_set_viewport(
                    cmd,
                    0,
                    std::slice::from_ref(
                        &vk::Viewport::builder()
                            .x(0.0)
                            .y(0.0)
                            .width(width as f32)
                            .height(height as f32)
                            .min_depth(0.0)
                            .max_depth(1.0),
                    ),
                );
            }

            // Draw the mesh
            unsafe {
                device.cmd_draw_indexed(
                    cmd,
                    mesh.indices.len() as u32,
                    1,
                    index_base,
                    vertex_base,
                    0,
                );
            }

            vertex_base += mesh.vertices.len() as i32;
            index_base += mesh.indices.len() as u32;
        }
    }

    fn get_vertex_desc() -> VertexInputDescription {
        let bindings = vec![vk::VertexInputBindingDescription::builder()
            .binding(0)
            .input_rate(vk::VertexInputRate::VERTEX)
            .stride(
                4 * std::mem::size_of::<f32>() as u32
                    + 4 * std::mem::size_of::<u8>() as u32,
            )
            .build()];

        let attributes = vec![
            // Position (Vec2 of f32s)
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .offset(0)
                .format(vk::Format::R32G32_SFLOAT)
                .build(),
            // UV (Vec4 of u8s)
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .offset(8)
                .format(vk::Format::R8G8B8A8_UNORM)
                .build(),
            // Color (Vec2 of f32s)
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(2)
                .offset(16)
                .format(vk::Format::R32G32_SFLOAT)
                .build(),
        ];

        VertexInputDescription {
            bindings,
            attributes,
            flags: vk::PipelineVertexInputStateCreateFlags::empty(),
        }
    }

    fn default_vertex_buffer_size() -> u64 {
        1024 * 1024 * 4
    }

    fn default_fragment_buffer_size() -> u64 {
        1024 * 1024 * 4
    }
}

struct ManagedTextures {
    textures: HashMap<egui::TextureId, Texture>,
}

impl ManagedTextures {
    fn create_sampler(device: &ash::Device) -> Result<vk::Sampler> {
        Ok(unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfo::builder()
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .anisotropy_enable(false)
                    .min_filter(vk::Filter::LINEAR)
                    .mag_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                    .min_lod(0.0)
                    .max_lod(vk::LOD_CLAMP_NONE),
                None,
            )?
        })
    }

    fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    fn update_textures(
        &mut self,
        cmd: vk::CommandBuffer,
        textures_delta: &egui::TexturesDelta,
        upload_context: &UploadContext,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<()> {
        for (id, image_delta) in &textures_delta.set {
            self.update_texture(
                cmd,
                *id,
                image_delta,
                upload_context,
                device,
                allocator,
                desc_allocator,
            )?;
        }
        for id in &textures_delta.free {
            self.free_texture(*id, device, allocator);
        }

        Ok(())
    }

    fn free_texture(
        &mut self,
        id: egui::TextureId,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) {
        if let Some(texture) = self.textures.remove(&id) {
            texture.cleanup(device, allocator)
        }
    }

    fn update_texture(
        &mut self,
        cmd: vk::CommandBuffer,
        texture_id: egui::TextureId,
        delta: &egui::epaint::ImageDelta,
        upload_context: &UploadContext,
        device: &ash::Device,
        allocator: &mut Allocator,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<()> {
        // Extract pixel data from egui
        let data: Vec<u8> = match &delta.image {
            egui::ImageData::Color(image) => {
                assert_eq!(
                    image.width() * image.height(),
                    image.pixels.len(),
                    "Mismatch between texture size and texel count",
                );
                image
                    .pixels
                    .iter()
                    .flat_map(|color| color.to_array())
                    .collect()
            }
            egui::ImageData::Font(image) => image
                .srgba_pixels(None)
                .flat_map(|color| color.to_array())
                .collect(),
        };

        // Create AllocatedImage with uninitialized GPU-only data
        let image = AllocatedImage::new(
            &AllocatedImageCreateInfo {
                format: vk::Format::R8G8B8A8_UNORM,
                extent: vk::Extent3D {
                    width: delta.image.width() as u32,
                    height: delta.image.height() as u32,
                    depth: 1,
                },
                usage_flags: vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::TRANSFER_SRC,
                aspect_flags: vk::ImageAspectFlags::COLOR,
                name: "egui managed texture".into(),
                desc_set: Some(
                    desc_allocator.allocate(device, "egui texture")?,
                ),
            },
            device,
            allocator,
        )?;

        // Upload data into image
        image.upload(&data, upload_context, device, allocator)?;

        // Create sampler
        let sampler = Self::create_sampler(device)?;

        // Create texture
        let texture = Texture::new(image, sampler, device)?;

        // Decide whether to register new texture or update existing texture
        // Update existing texture if font changed (delta pos exists)
        if let Some(pos) = delta.pos {
            let existing_texture = self.textures.get(&texture_id);
            if let Some(existing_texture) = existing_texture {
                existing_texture.image.copy_to_image(
                    cmd,
                    existing_texture.image.image,
                    vk::Extent2D {
                        width: delta.image.width() as u32,
                        height: delta.image.height() as u32,
                    },
                    device,
                );
            }
            texture.cleanup(device, allocator);
        // Otherwise, register new texture
        } else {
            if let Some(old_texture) = self.textures.remove(&texture_id) {
                old_texture.cleanup(device, allocator);
            }

            self.textures.insert(texture_id, texture);
        }

        Ok(())
    }

    pub fn cleanup(mut self, device: &ash::Device, allocator: &mut Allocator) {
        for (_, texture) in self.textures.drain() {
            texture.cleanup(device, allocator);
        }
    }
}

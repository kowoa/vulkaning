mod vkinit;
mod vkutils;

mod buffer;
mod core;
mod descriptors;
mod image;
pub mod queue_family_indices;
pub mod resources;
mod swapchain;
mod upload_context;

pub mod window;

use color_eyre::eyre::{eyre, OptionExt, Result};
use egui_ash::{AshRenderState, EguiCommand};
use glam::{Mat4, Vec3, Vec4};
use gpu_allocator::vulkan::Allocator;
use std::sync::{Arc, Mutex, MutexGuard};
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    keyboard::{Key, NamedKey},
};

use ash::vk;

use crate::renderer::{
    buffer::AllocatedBuffer, image::AllocatedImage,
    resources::camera::GpuCameraData,
};

use self::{
    core::Core,
    descriptors::{DescriptorAllocator, DescriptorLayoutBuilder},
    image::AllocatedImageCreateInfo,
    resources::{frame::Frame, scene::GpuSceneData, Resources},
    swapchain::Swapchain,
    upload_context::UploadContext,
    window::Window,
};

const FRAME_OVERLAP: u32 = 2;
const MAX_OBJECTS: u32 = 10000; // Max objects per frame

#[derive(Clone)]
pub struct Renderer {
    inner: Arc<Mutex<RendererInner>>,
}

impl Renderer {
    pub fn new(
        window: &Window,
        winit_window: Option<&winit::window::Window>,
    ) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(RendererInner::new(
                window,
                winit_window,
            )?)),
        })
    }

    pub fn draw_frame(
        &self,
        width: u32,
        height: u32,
        egui_cmd: Option<EguiCommand>,
    ) -> Result<u32> {
        self.inner
            .lock()
            .unwrap()
            .draw_frame(width, height, egui_cmd)
    }

    pub fn present_frame(&self, swapchain_image_index: u32) -> Result<()> {
        self.inner
            .lock()
            .unwrap()
            .present_frame(swapchain_image_index)
    }

    pub fn run_loop_without_egui(self, window: Window) -> Result<()> {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => {
                let inner = inner.into_inner()?;
                inner.run_loop_without_egui(window)
            }
            Err(_) => Err(eyre!("Failed to unwrap Arc<Mutex<RendererInner>>")),
        }
    }

    pub fn ash_render_state(&self) -> AshRenderState<Arc<Mutex<Allocator>>> {
        let inner = self.inner.lock().unwrap();
        AshRenderState {
            entry: inner.core.entry.clone(),
            instance: inner.core.instance.clone(),
            physical_device: inner.core.physical_device,
            device: inner.core.device.clone(),
            surface_loader: inner.core.surface_loader.clone(),
            swapchain_loader: inner.swapchain.swapchain_loader.clone(),
            queue: inner.core.graphics_queue,
            queue_family_index: inner
                .core
                .queue_family_indices
                .get_graphics_family()
                .unwrap(),
            command_pool: inner.command_pool,
            allocator: inner.core.get_allocator(),
        }
    }
}

struct RendererInner {
    core: Core,
    swapchain: Swapchain,
    resources: Resources,

    frame_number: u32,
    frames: Vec<Arc<Mutex<Frame>>>,
    command_pool: vk::CommandPool,

    scene_camera_buffer: AllocatedBuffer,

    upload_context: UploadContext,

    first_draw: bool,

    draw_image: AllocatedImage, // Image to render into
}

impl RendererInner {
    pub fn new(
        window: &Window,
        winit_window: Option<&winit::window::Window>,
    ) -> Result<Self> {
        log::info!("Initializing renderer ...");

        let mut core = Core::new(window, winit_window)?;
        let swapchain = if let Some(window) = winit_window {
            Swapchain::new(&mut core, window)?
        } else {
            let window =
                window.window.as_ref().ok_or_eyre("No window_found")?;
            Swapchain::new(&mut core, window)?
        };

        {
            let mut desc_allocator = core.get_desc_allocator_mut()?;
            Self::create_desc_set_layouts(&core.device, &mut desc_allocator)?;
        }

        let upload_context = UploadContext::new(
            &core.device,
            core.queue_family_indices.get_graphics_family()?,
            core.graphics_queue,
        )?;

        let draw_image = Self::create_draw_image(
            &mut core,
            window.width(),
            window.height(),
        )?;

        let resources =
            Resources::new(&mut core, &swapchain, &upload_context, window)?;

        let scene_camera_buffer = {
            let scene_size = core
                .pad_uniform_buffer_size(
                    std::mem::size_of::<GpuSceneData>() as u64
                ) as u32;
            let camera_size = core
                .pad_uniform_buffer_size(
                    std::mem::size_of::<GpuCameraData>() as u64
                ) as u32;
            let size = FRAME_OVERLAP * (scene_size + camera_size);
            let offsets =
                [0, scene_size, 2 * scene_size, 2 * scene_size + camera_size];

            let mut allocator = core.get_allocator_mut()?;
            let mut buffer = AllocatedBuffer::new(
                &core.device,
                &mut allocator,
                size as u64,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                "Scene-Camera Uniform Buffer",
                gpu_allocator::MemoryLocation::CpuToGpu,
            )?;
            buffer.set_offsets(offsets.to_vec());
            buffer
        };

        let command_pool = Self::create_command_pool(
            &core.device,
            core.queue_family_indices.get_graphics_family()?,
        )?;

        let frames = {
            let mut frames = Vec::with_capacity(FRAME_OVERLAP as usize);
            for _ in 0..FRAME_OVERLAP {
                // Call Frame constructor
                let frame =
                    Frame::new(&mut core, &scene_camera_buffer, &command_pool)?;

                frames.push(Arc::new(Mutex::new(frame)));
            }
            frames
        };

        Ok(Self {
            core,
            swapchain,
            resources,
            frame_number: 0,
            frames,
            command_pool,
            scene_camera_buffer,
            upload_context,
            first_draw: true,
            draw_image,
        })
    }

    fn run_loop_without_egui(self, window: Window) -> Result<()> {
        let event_loop = window.event_loop.ok_or_eyre("No event loop found")?;
        let window = window.window.ok_or_eyre("No window found")?;
        let mut renderer = Some(self);
        let mut close_requested = false;

        log::info!("Starting render loop ...");
        Ok(event_loop.run(move |event, elwt| match event {
            Event::WindowEvent { event, window_id }
                if window_id == window.id() =>
            {
                match event {
                    WindowEvent::CloseRequested => close_requested = true,
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key: key,
                                state: ElementState::Released,
                                ..
                            },
                        ..
                    } => {
                        if let Key::Named(NamedKey::Escape) = key.as_ref() {
                            close_requested = true
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        if let Some(r) = &mut renderer {
                            let size = &window.inner_size();
                            let swapchain_image_index = r
                                .draw_frame(size.width, size.height, None)
                                .unwrap();
                            window.pre_present_notify();
                            r.present_frame(swapchain_image_index).unwrap();
                        }
                    }
                    _ => (),
                }
            }
            Event::AboutToWait => {
                if close_requested {
                    renderer.take().unwrap().cleanup();
                    elwt.exit();
                } else {
                    window.request_redraw();
                }
            }
            _ => (),
        })?)
    }

    fn get_current_frame(&self) -> Result<MutexGuard<Frame>> {
        match self.frames[(self.frame_number % FRAME_OVERLAP) as usize].lock() {
            Ok(frame) => Ok(frame),
            Err(err) => Err(eyre!(err.to_string())),
        }
    }

    /// This function currently has absolutely no effect on the rendered image
    /// because the renderpass handles clearing the color and depth attachments.
    /// This means the renderpass's clear values will overwrite these ones.
    fn draw_background(&mut self, cmd: vk::CommandBuffer) {
        self.draw_image.transition_layout(
            cmd,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
            &self.core.device,
        );

        let flash = (self.frame_number as f32 / 120.0).sin().abs();
        let clear = vk::ClearColorValue {
            float32: [0.0, flash, 0.0, 1.0],
        };
        let clear_range =
            vkinit::image_subresource_range(vk::ImageAspectFlags::COLOR);
        unsafe {
            self.core.device.cmd_clear_color_image(
                cmd,
                self.draw_image.image,
                vk::ImageLayout::GENERAL,
                &clear,
                &[clear_range],
            );
        }
    }

    fn draw_frame(
        &mut self,
        width: u32,
        height: u32,
        mut egui_cmd: Option<EguiCommand>,
    ) -> Result<u32> {
        if self.first_draw {
            self.first_draw = false;
            // Update swapchain if it needs to be recreated
            let egui_cmd_take = egui_cmd.take();
            if let Some(mut cmd) = egui_cmd_take {
                cmd.update_swapchain(egui_ash::SwapchainUpdateInfo {
                    width,
                    height,
                    swapchain_images: self.swapchain.images.clone(),
                    surface_format: self.swapchain.image_format,
                });
                egui_cmd = Some(cmd);
            }
        }

        let (
            swapchain_image_index,
            cmd,
            render_semaphore,
            present_semaphore,
            render_fence,
        ) = {
            let frame = self.get_current_frame()?;
            let fences = [frame.render_fence];

            // Wait until GPU has finished rendering last frame (1 sec timeout)
            unsafe {
                self.core
                    .device
                    .wait_for_fences(&fences, true, 1000000000)?;
                self.core.device.reset_fences(&fences)?;
            }

            // Request image from swapchain (1 sec timeout)
            let (swapchain_image_index, _) = unsafe {
                self.swapchain.swapchain_loader.acquire_next_image(
                    self.swapchain.swapchain,
                    1000000000,
                    frame.present_semaphore,
                    vk::Fence::null(),
                )?
            };

            (
                swapchain_image_index,
                frame.command_buffer,
                frame.render_semaphore,
                frame.present_semaphore,
                frame.render_fence,
            )
        };

        // Begin command buffer recording
        {
            // Reset the command buffer to begin recording
            unsafe {
                self.core.device.reset_command_buffer(
                    cmd,
                    vk::CommandBufferResetFlags::empty(),
                )?;
            }

            // Begin command buffer recording
            let cmd_begin_info = vk::CommandBufferBeginInfo {
                flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                ..Default::default()
            };
            unsafe {
                self.core
                    .device
                    .begin_command_buffer(cmd, &cmd_begin_info)?;
            }
        }

        // Operations related to the draw image
        {
            // Clear the draw image with a background color
            self.draw_background(cmd);

            let device = &self.core.device;
            let swapchain_image =
                self.swapchain.images[swapchain_image_index as usize];

            // Transition the draw image and swapchain image into their correct transfer layouts
            self.draw_image.transition_layout(
                cmd,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                device,
            );
            vkutils::transition_image_layout(
                cmd,
                swapchain_image,
                vk::ImageAspectFlags::COLOR,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                device,
            );

            // Execute a copy from the draw image into the swapchain
            self.draw_image.copy_to_image(
                cmd,
                swapchain_image,
                self.swapchain.image_extent,
                device,
            );
        }

        {
            let clear_values = {
                // Make clear color from frame number
                let flash = (self.frame_number % 100) as f32 / 100.0;
                let clear = vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.0, 0.0, flash, 1.0],
                    },
                };

                // Make clear value for depth buffer
                let depth_clear = vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                };

                [clear, depth_clear]
            };

            // Start the main renderpass
            let rp = &self.resources.renderpasses[0];
            let rp_begin_info = vk::RenderPassBeginInfo {
                render_pass: rp.renderpass,
                render_area: vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D { width, height },
                },
                framebuffer: rp.framebuffers[swapchain_image_index as usize],
                clear_value_count: clear_values.len() as u32,
                p_clear_values: clear_values.as_ptr(),
                ..Default::default()
            };
            unsafe {
                self.core.device.cmd_begin_render_pass(
                    cmd,
                    &rp_begin_info,
                    vk::SubpassContents::INLINE,
                );
            }
        };

        // RENDERING COMMANDS START

        self.draw_render_objects(
            width,
            height,
            0,
            self.resources.render_objs.len(),
        )?;

        // RENDERING COMMANDS END

        unsafe {
            // Finalize the main renderpass
            self.core.device.cmd_end_render_pass(cmd);
        }
        // Record egui commands
        if let Some(egui_cmd) = egui_cmd {
            egui_cmd.record(cmd, swapchain_image_index as usize);
        }

        unsafe {
            // Finalize the main command buffer
            self.core.device.end_command_buffer(cmd)?;

            // Prepare submission to the graphics queue
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = vk::SubmitInfo {
                p_wait_dst_stage_mask: wait_stages.as_ptr(),
                wait_semaphore_count: 1,
                p_wait_semaphores: &present_semaphore,
                signal_semaphore_count: 1,
                p_signal_semaphores: &render_semaphore,
                command_buffer_count: 1,
                p_command_buffers: &cmd,
                ..Default::default()
            };
            self.core.device.queue_submit(
                self.core.graphics_queue,
                &[submit_info],
                render_fence,
            )?;
        }

        Ok(swapchain_image_index)
    }

    fn present_frame(&mut self, swapchain_image_index: u32) -> Result<()> {
        {
            let frame = self.get_current_frame()?;
            let present_info = vk::PresentInfoKHR {
                p_swapchains: &self.swapchain.swapchain,
                swapchain_count: 1,
                p_wait_semaphores: &frame.render_semaphore,
                wait_semaphore_count: 1,
                p_image_indices: &swapchain_image_index,
                ..Default::default()
            };

            unsafe {
                self.swapchain
                    .swapchain_loader
                    .queue_present(self.core.graphics_queue, &present_info)?;
            }
        }

        self.frame_number += 1;

        Ok(())
    }

    pub fn draw_render_objects(
        &mut self,
        width: u32,
        height: u32,
        first_index: usize,
        count: usize,
    ) -> Result<()> {
        let core = &self.core;
        let frame_index = self.frame_number % FRAME_OVERLAP;
        let scene_start_offset = core
            .pad_uniform_buffer_size(std::mem::size_of::<GpuSceneData>() as u64)
            * frame_index as u64;
        let camera_start_offset = core
            .pad_uniform_buffer_size(std::mem::size_of::<GpuSceneData>() as u64)
            * FRAME_OVERLAP as u64
            + core.pad_uniform_buffer_size(
                std::mem::size_of::<GpuCameraData>() as u64,
            ) * frame_index as u64;

        // Write into scene section of scene-camera uniform buffer
        {
            // Fill a GpuSceneData struct
            let framed = self.frame_number as f32 / 120.0;
            let scene_data = GpuSceneData {
                ambient_color: Vec4::new(framed.sin(), 0.0, framed.cos(), 1.0),
                ..Default::default()
            };

            // Copy GpuSceneData struct to buffer
            self.scene_camera_buffer
                .write(&[scene_data], scene_start_offset as usize)?;
        }

        // Write into camera section of scene-camera uniform buffer
        {
            // Fill a GpuCameraData struct
            let cam_pos = Vec3::new(0.0, 20.0, 0.0);
            //let cam_pos = Vec3::new(0.0, 40.0, 0.0);
            let view = Mat4::look_to_rh(
                cam_pos,
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 1.0, 0.0),
            );
            let mut proj = Mat4::perspective_rh(
                70.0,
                width as f32 / height as f32,
                0.1,
                200.0,
            );
            proj.y_axis.y *= -1.0;
            let cam_data = GpuCameraData {
                proj,
                view,
                viewproj: proj * view,
            };

            // Copy GpuCameraData struct to buffer
            self.scene_camera_buffer
                .write(&[cam_data], camera_start_offset as usize)?;
        }

        // Write into object storage buffer
        {
            //let rot = Mat4::from_rotation_y(self.frame_number as f32 / 240.0);
            let rot = Mat4::IDENTITY;
            let object_data = self
                .resources
                .render_objs
                .iter()
                .map(|obj| rot * obj.transform)
                .collect::<Vec<_>>();
            let mut frame = self.get_current_frame()?;
            frame.object_buffer.write(&object_data, 0)?;
        }

        let mut last_model_drawn = None;
        let mut last_material_drawn = None;
        for instance_index in first_index..(first_index + count) {
            let device = &core.device;
            let render_obj = &self.resources.render_objs[instance_index];
            let frame = self.get_current_frame()?;

            render_obj.draw(
                device,
                &frame,
                frame_index,
                &mut last_model_drawn,
                &mut last_material_drawn,
                &self.scene_camera_buffer,
                instance_index as u32,
            )?;
        }

        Ok(())
    }
    fn cleanup(mut self) {
        // Wait until all frames have finished rendering
        for frame in &self.frames {
            let frame = frame.lock().unwrap();
            unsafe {
                self.core
                    .device
                    .wait_for_fences(&[frame.render_fence], true, 1000000000)
                    .unwrap();
            }
        }

        {
            let device = &self.core.device;
            let mut allocator = self.core.get_allocator_mut().unwrap();

            self.upload_context.cleanup(device);

            self.resources.cleanup(device, &mut allocator);

            // Destroy command pool
            unsafe {
                device.destroy_command_pool(self.command_pool, None);
            }

            // Clean up all frames
            for _ in 0..self.frames.len() {
                let frame = self.frames.pop().unwrap();
                let frame = Arc::try_unwrap(frame).unwrap();
                let frame = frame.into_inner().unwrap();
                frame.cleanup(device, &mut allocator);
            }

            // Clean up buffers
            self.scene_camera_buffer.cleanup(device, &mut allocator);

            // Clean up swapchain
            self.swapchain.cleanup(device, &mut allocator);
        }

        // Clean up core Vulkan objects
        self.core.cleanup();
    }

    /// Helper function that creates a command pool
    fn create_command_pool(
        device: &ash::Device,
        graphics_family_index: u32,
    ) -> Result<vk::CommandPool> {
        let pool_info = vk::CommandPoolCreateInfo {
            queue_family_index: graphics_family_index,
            // Allow the pool to reset individual command buffers
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            ..Default::default()
        };
        let command_pool =
            unsafe { device.create_command_pool(&pool_info, None)? };
        Ok(command_pool)
    }

    fn create_desc_set_layouts(
        device: &ash::Device,
        desc_allocator: &mut DescriptorAllocator,
    ) -> Result<()> {
        let global_desc_set_layout = {
            // Binding 0 for GpuSceneData
            let scene_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
            );
            // Binding 1 for GpuCameraData
            let camera_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::ShaderStageFlags::VERTEX,
                1,
            );
            let bindings = [scene_bind, camera_bind];

            let set_info = vk::DescriptorSetLayoutCreateInfo {
                binding_count: bindings.len() as u32,
                flags: vk::DescriptorSetLayoutCreateFlags::empty(),
                p_bindings: bindings.as_ptr(),
                ..Default::default()
            };
            unsafe { device.create_descriptor_set_layout(&set_info, None)? }
        };

        let object_desc_set_layout = {
            // Binding 0 for GpuObjectData
            let object_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::STORAGE_BUFFER,
                vk::ShaderStageFlags::VERTEX,
                0,
            );

            let set_info = vk::DescriptorSetLayoutCreateInfo {
                binding_count: 1,
                p_bindings: &object_bind,
                ..Default::default()
            };
            unsafe { device.create_descriptor_set_layout(&set_info, None)? }
        };

        let single_texture_desc_set_layout = {
            let texture_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::ShaderStageFlags::FRAGMENT,
                0,
            );

            let set_info = vk::DescriptorSetLayoutCreateInfo {
                binding_count: 1,
                p_bindings: &texture_bind,
                ..Default::default()
            };
            unsafe { device.create_descriptor_set_layout(&set_info, None)? }
        };

        desc_allocator.add_layout("global", global_desc_set_layout);
        desc_allocator.add_layout("object", object_desc_set_layout);
        desc_allocator
            .add_layout("single texture", single_texture_desc_set_layout);

        Ok(())
    }

    /// Helper function that creates the descriptor pool and descriptor sets
    fn create_draw_image(
        core: &mut Core,
        width: u32,
        height: u32,
    ) -> Result<AllocatedImage> {
        let draw_image_desc_set = {
            let layout = DescriptorLayoutBuilder::new()
                .add_binding(
                    0,
                    vk::DescriptorType::STORAGE_IMAGE,
                    vk::ShaderStageFlags::COMPUTE,
                )
                .build(&core.device)?;
            let mut desc_allocator = core.get_desc_allocator_mut()?;
            desc_allocator.add_layout("draw image", layout);
            let set = desc_allocator.allocate(&core.device, "draw image")?;
            set
        };

        let draw_image = {
            let extent = vk::Extent3D {
                width,
                height,
                depth: 1,
            };
            let usage_flags = vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::COLOR_ATTACHMENT;
            let create_info = AllocatedImageCreateInfo {
                format: vk::Format::R16G16B16A16_SFLOAT,
                extent,
                usage_flags,
                aspect_flags: vk::ImageAspectFlags::COLOR,
                name: "Draw image".into(),
                desc_set: Some(draw_image_desc_set),
            };
            let mut allocator = core.get_allocator_mut()?;
            AllocatedImage::new(&create_info, &core.device, &mut allocator)?
        };

        let draw_image_info = [vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::GENERAL)
            .image_view(draw_image.view)
            .build()];
        let draw_image_write = vk::WriteDescriptorSet::builder()
            .dst_binding(0)
            .dst_set(draw_image_desc_set)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(&draw_image_info)
            .build();
        unsafe {
            core.device.update_descriptor_sets(&[draw_image_write], &[]);
        }

        Ok(draw_image)
    }
}

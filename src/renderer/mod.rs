mod utils;
mod vkinit;

mod core;
mod memory;
pub mod resources;
mod swapchain;

pub mod window;

use color_eyre::eyre::{OptionExt, Result};
use std::{cell::RefCell, mem::ManuallyDrop, rc::Rc};

use ash::vk;

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    keyboard::{Key, NamedKey},
};

use crate::renderer::resources::camera::GpuCameraData;

use self::{
    core::Core,
    memory::AllocatedBuffer,
    resources::{frame::Frame, scene::GpuSceneData, Resources},
    swapchain::Swapchain,
    window::Window,
};

const FRAME_OVERLAP: u32 = 2;
const MAX_OBJECTS: u32 = 10000; // Max objects per frame

pub struct Renderer {
    window: Option<Window>,
    core: Core,
    swapchain: Swapchain,
    resources: Resources,

    frame_number: u32,
    frames: Vec<Rc<RefCell<Frame>>>,

    global_desc_set_layout: vk::DescriptorSetLayout,
    object_desc_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,

    scene_params_buffer: AllocatedBuffer,
}

impl Renderer {
    pub fn new(window: Window) -> Result<Self> {
        log::info!("Initializing renderer ...");

        let mut core = Core::new(&window)?;
        let swapchain = Swapchain::new(&mut core, &window)?;
        let (global_desc_set_layout, object_desc_set_layout, descriptor_pool) =
            Self::create_descriptors(&core)?;
        let resources = Resources::new(
            &mut core,
            &swapchain,
            &global_desc_set_layout,
            &object_desc_set_layout,
        )?;

        let scene_params_buffer = {
            let size = FRAME_OVERLAP as u64
                * core.pad_uniform_buffer_size(
                    std::mem::size_of::<GpuSceneData>() as u64,
                );
            AllocatedBuffer::new(
                &core.device,
                &mut core.allocator,
                size,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                "Uniform Scene Params Buffer",
                gpu_allocator::MemoryLocation::CpuToGpu,
            )?
        };

        let frames = {
            let mut frames = Vec::with_capacity(FRAME_OVERLAP as usize);
            for _ in 0..FRAME_OVERLAP {
                // Call Frame constructor
                let frame = Frame::new(
                    &mut core,
                    &descriptor_pool,
                    &global_desc_set_layout,
                    &object_desc_set_layout,
                    &scene_params_buffer,
                )?;

                frames.push(Rc::new(RefCell::new(frame)));
            }
            frames
        };

        Ok(Self {
            window: Some(window),
            core,
            swapchain,
            resources,
            frame_number: 0,
            frames,
            global_desc_set_layout,
            object_desc_set_layout,
            descriptor_pool,
            scene_params_buffer,
        })
    }

    pub fn run_loop(mut self) -> Result<()> {
        let window = self
            .window
            .take()
            .ok_or_eyre("Renderer does not own a Window")?;
        let event_loop = window.event_loop;
        let window = window.window;
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
                    } => match key.as_ref() {
                        Key::Named(NamedKey::Escape) => close_requested = true,
                        _ => (),
                    },
                    WindowEvent::RedrawRequested => {
                        if let Some(r) = &mut renderer {
                            let swapchain_image_index =
                                r.draw_frame(&window).unwrap();
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

    fn get_current_frame(&self) -> Rc<RefCell<Frame>> {
        self.frames[(self.frame_number % FRAME_OVERLAP) as usize].clone()
    }

    fn draw_frame(&mut self, window: &winit::window::Window) -> Result<u32> {
        let frame = self.get_current_frame();
        let mut frame = frame.borrow_mut();
        unsafe {
            let fences = [frame.render_fence];

            // Wait until GPU has finished rendering last frame (1 sec timeout)
            self.core
                .device
                .wait_for_fences(&fences, true, 1000000000)?;
            self.core.device.reset_fences(&fences)?;

            // Request image from swapchain (1 sec timeout)
            let (swapchain_image_index, _) =
                self.swapchain.swapchain_loader.acquire_next_image(
                    self.swapchain.swapchain,
                    1000000000,
                    frame.present_semaphore,
                    vk::Fence::null(),
                )?;

            // Reset the command buffer to begin recording
            let cmd = frame.command_buffer;
            self.core.device.reset_command_buffer(
                cmd,
                vk::CommandBufferResetFlags::empty(),
            )?;

            // Begin command buffer recording
            let cmd_begin_info = vk::CommandBufferBeginInfo {
                flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                ..Default::default()
            };
            self.core
                .device
                .begin_command_buffer(cmd, &cmd_begin_info)?;

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
                    extent: vk::Extent2D {
                        width: window.inner_size().width,
                        height: window.inner_size().height,
                    },
                },
                framebuffer: rp.framebuffers[swapchain_image_index as usize],
                clear_value_count: clear_values.len() as u32,
                p_clear_values: clear_values.as_ptr(),
                ..Default::default()
            };
            self.core.device.cmd_begin_render_pass(
                cmd,
                &rp_begin_info,
                vk::SubpassContents::INLINE,
            );

            // RENDERING COMMANDS START

            self.resources.draw_render_objects(
                &mut self.core,
                &cmd,
                window,
                0,
                self.resources.render_objs.len(),
                &mut frame,
                self.frame_number,
                &mut self.scene_params_buffer,
            )?;

            // RENDERING COMMANDS END

            // Finalize the main renderpass
            self.core.device.cmd_end_render_pass(cmd);
            // Finalize the main command buffer
            self.core.device.end_command_buffer(cmd)?;

            // Prepare submission to the graphics queue
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = vk::SubmitInfo {
                p_wait_dst_stage_mask: wait_stages.as_ptr(),
                wait_semaphore_count: 1,
                p_wait_semaphores: &frame.present_semaphore,
                signal_semaphore_count: 1,
                p_signal_semaphores: &frame.render_semaphore,
                command_buffer_count: 1,
                p_command_buffers: &cmd,
                ..Default::default()
            };
            self.core.device.queue_submit(
                self.core.graphics_queue,
                &[submit_info],
                frame.render_fence,
            )?;

            Ok(swapchain_image_index)
        }
    }

    fn present_frame(&mut self, swapchain_image_index: u32) -> Result<()> {
        let frame = self.get_current_frame();
        let present_info = vk::PresentInfoKHR {
            p_swapchains: &self.swapchain.swapchain,
            swapchain_count: 1,
            p_wait_semaphores: &frame.borrow().render_semaphore,
            wait_semaphore_count: 1,
            p_image_indices: &swapchain_image_index,
            ..Default::default()
        };

        unsafe {
            self.swapchain
                .swapchain_loader
                .queue_present(self.core.graphics_queue, &present_info)?;
        }

        self.frame_number += 1;

        Ok(())
    }

    fn cleanup(mut self) {
        // Wait until all frames have finished rendering
        for frame in &self.frames {
            unsafe {
                self.core
                    .device
                    .wait_for_fences(
                        &[frame.borrow().render_fence],
                        true,
                        1000000000,
                    )
                    .unwrap();
            }
        }

        let device = &self.core.device;
        let allocator = &mut self.core.allocator;

        unsafe {
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(
                self.object_desc_set_layout,
                None,
            );
            device.destroy_descriptor_set_layout(
                self.global_desc_set_layout,
                None,
            );
        }
        self.resources.cleanup(device, allocator);

        // Clean up all frames
        for frame in self.frames {
            let frame = Rc::try_unwrap(frame);
            let frame = frame.expect("Failed to cleanup frame");
            frame.into_inner().cleanup(device, allocator);
        }

        // Clean up buffers
        self.scene_params_buffer.cleanup(device, allocator);

        self.swapchain.cleanup(device, &mut self.core.allocator);

        // We need to do this because the allocator doesn't destroy all
        // memory blocks (VkDeviceMemory) until it is dropped.
        unsafe {
            ManuallyDrop::drop(&mut self.core.allocator);
        }
        self.core.cleanup();
    }

    fn create_descriptors(
        core: &Core,
    ) -> Result<(
        vk::DescriptorSetLayout,
        vk::DescriptorSetLayout,
        vk::DescriptorPool,
    )> {
        let global_desc_set_layout = {
            // Binding 0 for GpuCameraData
            let camera_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::UNIFORM_BUFFER,
                vk::ShaderStageFlags::VERTEX,
                0,
            );
            // Binding 1 for GpuSceneData
            let scene_bind = vkinit::descriptor_set_layout_binding(
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                1,
            );
            let bindings = [camera_bind, scene_bind];

            let set_info = vk::DescriptorSetLayoutCreateInfo {
                binding_count: bindings.len() as u32,
                flags: vk::DescriptorSetLayoutCreateFlags::empty(),
                p_bindings: bindings.as_ptr(),
                ..Default::default()
            };
            unsafe {
                core.device.create_descriptor_set_layout(&set_info, None)?
            }
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
            unsafe {
                core.device.create_descriptor_set_layout(&set_info, None)?
            }
        };

        let descriptor_pool = {
            // Create a descriptor pool that will hold 10 uniform buffers
            // and 10 dynamic uniform buffers
            // and 10 storage buffers
            let sizes = vec![
                vk::DescriptorPoolSize {
                    // For the camera buffer
                    ty: vk::DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: 10,
                },
                vk::DescriptorPoolSize {
                    // For the scene params buffer
                    ty: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                    descriptor_count: 10,
                },
                vk::DescriptorPoolSize {
                    // For the object buffer
                    ty: vk::DescriptorType::STORAGE_BUFFER,
                    descriptor_count: 10,
                },
            ];
            let pool_info = vk::DescriptorPoolCreateInfo {
                max_sets: 10,
                pool_size_count: sizes.len() as u32,
                p_pool_sizes: sizes.as_ptr(),
                ..Default::default()
            };
            unsafe { core.device.create_descriptor_pool(&pool_info, None)? }
        };

        Ok((
            global_desc_set_layout,
            object_desc_set_layout,
            descriptor_pool,
        ))
    }
}

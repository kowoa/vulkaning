mod utils;
mod vk_initializers;

mod assets;
mod core;
mod memory;
mod swapchain;

use std::mem::ManuallyDrop;

use ash::vk;

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};

use self::{
    assets::{frame::Frame, Assets},
    core::Core,
    swapchain::Swapchain,
};

const FRAME_OVERLAP: u32 = 2;

pub struct Renderer {
    core: Core,
    swapchain: Swapchain,
    assets: Assets,

    frame_number: u32,
}

impl Renderer {
    pub fn new(
        window: &winit::window::Window,
        event_loop: &winit::event_loop::EventLoop<()>,
    ) -> anyhow::Result<Self> {
        let mut core = Core::new(window, event_loop)?;
        let swapchain = Swapchain::new(&mut core, window)?;
        let assets = Assets::new(&mut core, &swapchain, window)?;

        Ok(Self {
            core,
            swapchain,
            assets,
            frame_number: 0,
        })
    }

    pub fn render_loop(
        self,
        window: winit::window::Window,
        event_loop: EventLoop<()>,
    ) -> anyhow::Result<()> {
        let mut close_requested = false;
        let mut renderer = Some(self);
        let frames = {
            let mut frames = Vec::with_capacity(FRAME_OVERLAP as usize);
            let mut core = renderer.unwrap().core;
            let assets = renderer.unwrap().assets;
            let graphics_family_index =
                core.queue_family_indices.graphics_family.unwrap();
            for _ in 0..FRAME_OVERLAP {
                // Allocate one descriptor set for each frame
                let descriptor_set = {
                    let info = vk::DescriptorSetAllocateInfo {
                        descriptor_pool: assets.descriptor_pool,
                        descriptor_set_count: 1,
                        p_set_layouts: &assets.global_set_layout,
                        ..Default::default()
                    };
                    unsafe { core.device.allocate_descriptor_sets(&info)?[0] }
                };

                // Call Frame constructor
                let frame = Frame::new(
                    &core.device,
                    &mut core.allocator,
                    graphics_family_index,
                    assets.descriptor_pool,
                    assets.global_set_layout,
                )?;

                frames.push(frame);
            }
            frames
        };

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
                            let frame = r.get_current_frame(&frames);
                            let swapchain_image_index =
                                r.draw_frame(frame, &window).unwrap();
                            window.pre_present_notify();
                            r.present_frame(frame, swapchain_image_index).unwrap();
                        }
                    }
                    _ => (),
                }
            }
            Event::AboutToWait => {
                if close_requested {
                    renderer.take().unwrap().cleanup(frames);
                    elwt.exit();
                } else {
                    window.request_redraw();
                }
            }
            _ => (),
        })?)
    }

    fn get_current_frame(&self, frames: &Vec<Frame>) -> &mut Frame {
        &mut frames[(self.frame_number % FRAME_OVERLAP) as usize]
    }

    fn draw_frame(
        &self,
        frame: &mut Frame,
        window: &winit::window::Window,
    ) -> anyhow::Result<u32> {
        unsafe {
            let device = &self.core.device;
            let fences = [frame.render_fence];

            // Wait until GPU has finished rendering last frame (1 sec timeout)
            device.wait_for_fences(&fences, true, 1000000000)?;
            device.reset_fences(&fences)?;

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
            device.reset_command_buffer(
                cmd,
                vk::CommandBufferResetFlags::empty(),
            )?;

            // Begin command buffer recording
            let cmd_begin_info = vk::CommandBufferBeginInfo {
                flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                ..Default::default()
            };
            device.begin_command_buffer(cmd, &cmd_begin_info)?;

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
            let rp = &self.assets.renderpasses[0];
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
            device.cmd_begin_render_pass(
                cmd,
                &rp_begin_info,
                vk::SubpassContents::INLINE,
            );

            // RENDERING COMMANDS START

            self.assets.draw_render_objects(
                device,
                &cmd,
                window,
                0,
                self.assets.render_objs.len(),
                frame,
            );

            // RENDERING COMMANDS END

            // Finalize the main renderpass
            device.cmd_end_render_pass(cmd);
            // Finalize the main command buffer
            device.end_command_buffer(cmd)?;

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
            device.queue_submit(
                self.core.graphics_queue,
                &[submit_info],
                frame.render_fence,
            )?;

            Ok(swapchain_image_index)
        }
    }

    fn present_frame(
        &mut self,
        frame: &Frame,
        swapchain_image_index: u32,
    ) -> anyhow::Result<()> {
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

        self.frame_number += 1;

        Ok(())
    }

    fn cleanup(mut self, frames: Vec<Frame>) {
        // Wait until all frames have finished rendering
        for frame in &frames {
            unsafe {
                self.core
                    .device
                    .wait_for_fences(&[frame.render_fence], true, 1000000000)
                    .unwrap();
            }
        }

        let device = &self.core.device;
        self.assets.cleanup(device, &mut self.core.allocator);
        for frame in frames {
            frame.cleanup(device);
        }
        self.swapchain.cleanup(device, &mut self.core.allocator);

        // We need to do this because the allocator doesn't destroy all
        // memory blocks (VkDeviceMemory) until it is dropped.
        unsafe {
            ManuallyDrop::drop(&mut self.core.allocator);
        }
        self.core.cleanup();
    }
}

pub fn create_window() -> anyhow::Result<(Window, EventLoop<()>)> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Vulkaning")
        .with_inner_size(winit::dpi::LogicalSize::new(800, 600))
        .with_resizable(false)
        .build(&event_loop)?;

    Ok((window, event_loop))
}

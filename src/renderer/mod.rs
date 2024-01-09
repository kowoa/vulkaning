mod vk_initializers;
mod vk_types;
mod utils;

mod swapchain;
mod commands;
mod core;
mod sync_objs;
mod assets;

use ash::vk;

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};

use self::{assets::Assets, swapchain::Swapchain, core::Core, commands::Commands, sync_objs::SyncObjs};

pub struct Renderer {
    core: Core,
    swapchain: Swapchain,
    commands: Commands,
    sync_objs: SyncObjs,
    assets: Assets,

    frame_number: u32,
    selected_shader: i32,
    destroyed: bool,
}

impl Renderer {
    pub fn new(
        window: &winit::window::Window,
        event_loop: &winit::event_loop::EventLoop<()>,
    ) -> anyhow::Result<Self> {
        let core = Core::new(window, event_loop)?;
        let swapchain = Swapchain::new(&core, window)?;
        let commands = Commands::new(&core)?;
        let sync_objs = SyncObjs::new(&core)?;
        let assets = Assets::new(&core, &swapchain, window)?;

        Ok(Self {
            core,
            swapchain,
            commands,
            sync_objs,
            assets,
            frame_number: 0,
            selected_shader: 0,
            destroyed: false,
        })
    }

    pub fn render_loop(
        &mut self,
        window: winit::window::Window,
        event_loop: EventLoop<()>,
    ) -> anyhow::Result<()> {
        Ok(event_loop.run(move |event, elwt| match event {
            Event::WindowEvent { event, window_id }
                if window_id == window.id() =>
            {
                match event {
                    WindowEvent::CloseRequested => {
                        self.destroy();
                        elwt.exit();
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key: key,
                                state: ElementState::Released,
                                ..
                            },
                        ..
                    } => match key.as_ref() {
                        Key::Named(NamedKey::Escape) => {
                            self.destroy();
                            elwt.exit();
                        }
                        Key::Named(NamedKey::Space) => {
                            self.selected_shader =
                                (self.selected_shader + 1) % 2;
                        }
                        _ => (),
                    },
                    WindowEvent::RedrawRequested => {
                        let swapchain_image_index =
                            self.draw_frame(&window).unwrap();
                        window.pre_present_notify();
                        self.present_frame(swapchain_image_index).unwrap();
                    }
                    _ => (),
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => (),
        })?)
    }

    fn draw_frame(
        &self,
        window: &winit::window::Window,
    ) -> anyhow::Result<u32> {
        unsafe {
            let device = &self.core.device;
            let fences = [self.sync_objs.render_fence];

            // Wait until GPU has finished rendering last frame (1 sec timeout)
            device.wait_for_fences(&fences, true, 1000000000)?;
            device.reset_fences(&fences)?;

            // Request image from swapchain (1 sec timeout)
            let (swapchain_image_index, _) =
                self.swapchain.swapchain_loader.acquire_next_image(
                    self.swapchain.swapchain,
                    1000000000,
                    self.sync_objs.present_semaphore,
                    vk::Fence::null(),
                )?;

            // Reset the command buffer to begin recording
            let cmd = self.commands.main_command_buffer;
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

            // Make clear color from frame number
            let flash = (self.frame_number % 100) as f32 / 100.0;
            let clear = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, flash, 1.0],
                },
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
                framebuffer: rp.framebuffers
                    [swapchain_image_index as usize],
                clear_value_count: 1,
                p_clear_values: &clear,
                ..Default::default()
            };
            device.cmd_begin_render_pass(
                cmd,
                &rp_begin_info,
                vk::SubpassContents::INLINE,
            );

            // RENDERING COMMANDS START

            if self.selected_shader == 0 {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.assets.pipelines[0].pipeline,
                );
            } else {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.assets.pipelines[1].pipeline,
                );
            }
            device.cmd_draw(cmd, 3, 1, 0, 0);

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
                p_wait_semaphores: &self.sync_objs.present_semaphore,
                signal_semaphore_count: 1,
                p_signal_semaphores: &self.sync_objs.render_semaphore,
                command_buffer_count: 1,
                p_command_buffers: &cmd,
                ..Default::default()
            };
            device.queue_submit(
                self.core.graphics_queue,
                &[submit_info],
                self.sync_objs.render_fence,
            )?;

            Ok(swapchain_image_index)
        }
    }

    fn present_frame(
        &mut self,
        swapchain_image_index: u32,
    ) -> anyhow::Result<()> {
        let present_info = vk::PresentInfoKHR {
            p_swapchains: &self.swapchain.swapchain,
            swapchain_count: 1,
            p_wait_semaphores: &self.sync_objs.render_semaphore,
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

    fn destroy(&mut self) {
        if self.destroyed {
            return;
        }

        unsafe {
            self.core
                .device
                .wait_for_fences(
                    &[self.sync_objs.render_fence],
                    true,
                    1000000000,
                )
                .unwrap();
        }

        let device = &self.core.device;
        self.assets.destroy(device);
        self.sync_objs.destroy(device);
        self.commands.destroy(device);
        self.swapchain.destroy(device);
        self.core.destroy();

        self.destroyed = true;
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.destroy();
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

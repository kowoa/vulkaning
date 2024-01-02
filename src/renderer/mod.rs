mod shader;

mod vk_common;
mod vk_initializers;
mod vk_utils;

mod vk_command_objs;
mod vk_core_objs;
mod vk_pipeline_objs;
mod vk_renderpass_objs;
mod vk_swapchain_objs;
mod vk_sync_objs;

use ash::vk;
use vk_command_objs::VkCommandObjs;
use vk_core_objs::VkCoreObjs;
use vk_pipeline_objs::VkPipelineObjs;
use vk_renderpass_objs::VkRenderpassObjs;
use vk_swapchain_objs::VkSwapchainObjs;
use vk_sync_objs::VkSyncObjs;

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};

pub struct Renderer {
    core_objs: VkCoreObjs,
    swapchain_objs: VkSwapchainObjs,
    command_objs: VkCommandObjs,
    renderpass_objs: VkRenderpassObjs,
    sync_objs: VkSyncObjs,
    pipeline_objs: VkPipelineObjs,

    frame_number: u32,
    destroyed: bool,
}

impl Renderer {
    pub fn new(
        window: &winit::window::Window,
        event_loop: &winit::event_loop::EventLoop<()>,
    ) -> anyhow::Result<Self> {
        let core_objs = VkCoreObjs::new(window, event_loop)?;
        let swapchain_objs = VkSwapchainObjs::new(&core_objs, window)?;
        let command_objs = VkCommandObjs::new(&core_objs)?;
        let renderpass_objs =
            VkRenderpassObjs::new(&core_objs, &swapchain_objs, window)?;
        let sync_objs = VkSyncObjs::new(&core_objs)?;
        println!("before pipeline");
        let pipeline_objs =
            VkPipelineObjs::new(&core_objs, &swapchain_objs, &renderpass_objs)?;
        println!("after pipeline");

        Ok(Self {
            core_objs,
            swapchain_objs,
            command_objs,
            renderpass_objs,
            sync_objs,
            pipeline_objs,
            frame_number: 0,
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
            let device = &self.core_objs.device;
            let fences = [self.sync_objs.render_fence];

            // Wait until GPU has finished rendering last frame (1 sec timeout)
            device.wait_for_fences(&fences, true, 1000000000)?;
            device.reset_fences(&fences)?;

            // Request image from swapchain (1 sec timeout)
            let (swapchain_image_index, _) =
                self.swapchain_objs.swapchain_loader.acquire_next_image(
                    self.swapchain_objs.swapchain,
                    1000000000,
                    self.sync_objs.present_semaphore,
                    vk::Fence::null(),
                )?;

            // Reset the command buffer to begin recording
            let cmd = self.command_objs.main_command_buffer;
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
            let rp_begin_info = vk::RenderPassBeginInfo {
                render_pass: self.renderpass_objs.renderpass,
                render_area: vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: window.inner_size().width,
                        height: window.inner_size().height,
                    },
                },
                framebuffer: self.renderpass_objs.framebuffers
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

            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline_objs.pipeline);
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
                self.core_objs.graphics_queue,
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
            p_swapchains: &self.swapchain_objs.swapchain,
            swapchain_count: 1,
            p_wait_semaphores: &self.sync_objs.render_semaphore,
            wait_semaphore_count: 1,
            p_image_indices: &swapchain_image_index,
            ..Default::default()
        };

        unsafe {
            self.swapchain_objs
                .swapchain_loader
                .queue_present(self.core_objs.graphics_queue, &present_info)?;
        }

        self.frame_number += 1;

        Ok(())
    }

    pub fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.sync_objs.destroy(&self.core_objs);
        self.renderpass_objs.destroy(&self.core_objs);
        self.command_objs.destroy(&self.core_objs);
        self.swapchain_objs.destroy(&self.core_objs);
        self.core_objs.destroy();
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

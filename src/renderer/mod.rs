mod vk_core;
mod vk_utils;
mod vk_initializers;

use vk_core::VkCoreStructures;

use winit::{event_loop::EventLoop, window::{Window, WindowBuilder}, keyboard::{NamedKey, Key}, event::{ElementState, KeyEvent, WindowEvent, Event}};

pub struct Renderer {
    vk_core: VkCoreStructures,
}

impl Renderer {
    pub fn new(
        window: &winit::window::Window,
        event_loop: &winit::event_loop::EventLoop<()>,
    ) -> anyhow::Result<Self> {
        let vk_core = VkCoreStructures::new(window, event_loop)?;

        Ok(Self {
            vk_core,
        })
    }

    pub fn render_loop(&self,
        window: winit::window::Window,
        event_loop: EventLoop<()>
    ) -> anyhow::Result<()> {
        Ok(event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent {
                    event,
                    window_id
                } if window_id == window.id() => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key: key,
                                state: ElementState::Released,
                                ..
                            },
                        ..
                    } => {
                        match key.as_ref() {
                            Key::Named(NamedKey::Escape) => elwt.exit(),
                            _ => ()
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        self.draw_frame();
                        window.pre_present_notify();
                        self.present_frame();
                    }
                    _ => ()
                },
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => ()
            }
        })?)
    }

    fn draw_frame(&self) {
        log::info!("Drawing frame ...");
    }

    fn present_frame(&self) {
        log::info!("Presenting frame ...");
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.vk_core.destroy();
        }
    }
}

pub fn create_window() -> anyhow::Result<(Window, EventLoop<()>)> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Vulkaning")
        .with_inner_size(winit::dpi::LogicalSize::new(
            800,
            600,
        ))
        .with_resizable(false)
        .build(&event_loop)?;

    Ok((window, event_loop))
}

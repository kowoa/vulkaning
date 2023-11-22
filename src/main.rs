use vulkaning::Renderer;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
    event::{Event, WindowEvent, ElementState, KeyEvent},
    keyboard::{NamedKey, Key},
};

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    let (window, event_loop) = create_window()?;
    let renderer = Renderer::new(&window, &event_loop)?;

    log::info!("Starting render loop ...");
    event_loop.run(move |event, elwt| {
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
                    renderer.draw_frame();
                    window.pre_present_notify();
                    renderer.present_frame();
                }
                _ => ()
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => ()
        }
    })?;

    
    Ok(())
}

fn create_window() -> anyhow::Result<(Window, EventLoop<()>)> {
    log::info!("Creating window ...");

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

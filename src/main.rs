use vulkaning::Renderer;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    let (window, event_loop) = create_window()?;
    let renderer = Renderer::new();

    log::info!("Starting render loop ...");
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

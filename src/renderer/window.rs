use color_eyre::eyre::Result;

pub struct Window {
    pub window: winit::window::Window,
    pub event_loop: winit::event_loop::EventLoop<()>,
    pub width: u32,
    pub height: u32,
}

impl Window {
    pub fn new() -> Result<Self> {
        log::info!("Creating window ...");

        let width = 800;
        let height = 600;
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = winit::window::WindowBuilder::new()
            .with_title("Vulkaning")
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_resizable(false)
            .build(&event_loop)?;

        Ok(Self {
            window,
            event_loop,
            width,
            height,
        })
    }
}

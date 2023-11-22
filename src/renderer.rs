pub struct Renderer;

impl Renderer {
    pub fn new() -> Self {
        log::info!("Initializing renderer ...");
        Self {}
    }

    pub fn draw_frame(&self) {
        log::info!("Drawing frame ...");
    }

    pub fn present_frame(&self) {
        log::info!("Presenting frame ...");
    }
}

use std::process::ExitCode;

use crate::renderer::{window::Window, Renderer};
use color_eyre::eyre::Result;

use super::{App, AppType};

impl AppType for WinitApp {}
pub struct WinitApp;

impl App<WinitApp> {
    pub fn new() -> Result<Self> {
        let window = Window::new_without_egui()?;
        let renderer = Renderer::new(&window)?;
        let extra = WinitApp {};
        Ok(Self {
            renderer,
            window,
            extra,
        })
    }

    pub fn run(self) -> Result<ExitCode> {
        self.renderer.run_loop_without_egui(self.window)?;
        Ok(ExitCode::SUCCESS)
    }
}

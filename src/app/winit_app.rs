use std::process::ExitCode;

use crate::renderer::{window::Window, Renderer};
use color_eyre::eyre::Result;

use super::{App, AppType};

impl AppType for WinitApp {}
pub struct WinitApp {
    renderer: Renderer,
    window: Window,
}

impl App<WinitApp> {
    pub fn new() -> Result<Self> {
        let window = Window::new_without_egui()?;
        let renderer = Renderer::new(&window, None)?;
        let inner = WinitApp { renderer, window };
        Ok(Self { inner: Some(inner) })
    }

    pub fn run(self) -> Result<ExitCode> {
        // Safe to unwrap as inner is guaranteed to be Some
        let inner = self.inner.unwrap();
        inner.renderer.run_loop_without_egui(inner.window)?;
        Ok(ExitCode::SUCCESS)
    }
}

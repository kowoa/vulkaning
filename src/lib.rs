mod renderer;
use renderer::*;

use color_eyre::eyre::Result;

use crate::window::Window;

pub fn run() -> Result<()> {
    color_eyre::install()?;
    env_logger::init();

    let window = Window::new()?;
    let renderer = Renderer::new(window)?;
    renderer.run_loop()?;

    Ok(())
}

mod renderer;
use renderer::*;

use color_eyre::eyre::Result;

pub fn run() -> Result<()> {
    color_eyre::install()?;
    env_logger::init();

    log::info!("Creating window ...");
    let (window, event_loop) = create_window()?;

    log::info!("Initializing renderer ...");
    let renderer = Renderer::new(&window, &event_loop)?;

    log::info!("Starting render loop ...");
    renderer.run_loop(window, event_loop)?;

    Ok(())
}

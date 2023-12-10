use vulkaning::{Renderer, create_window};

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    log::info!("Creating window ...");
    let (window, event_loop) = create_window()?;

    log::info!("Initializing renderer ...");
    let renderer = Renderer::new(&window, &event_loop)?;

    log::info!("Starting render loop ...");
    //renderer.render_loop(window, event_loop)?;
    
    Ok(())
}


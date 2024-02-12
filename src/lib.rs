mod renderer;
use renderer::{resources::shader::SHADERBUILD_DIR, *};

use color_eyre::eyre::{eyre, Result};

use crate::window::Window;

pub fn run() -> Result<()> {
    color_eyre::install()?;
    env_logger::init();

    // Set shader build directory from command line args
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 {
        return Err(eyre!("Too many args"));
    } else if args.len() == 2 {
        unsafe { SHADERBUILD_DIR = Some(args[1].clone()) };
    } else {
        let dir = std::env::var("SHADER_BUILD_DIR")
            .unwrap_or_else(|_| "./shaderbuild".to_string());
        unsafe { SHADERBUILD_DIR = Some(dir) };
    }

    let window = Window::new()?;
    let renderer = Renderer::new(window)?;
    renderer.run_loop()?;

    Ok(())
}

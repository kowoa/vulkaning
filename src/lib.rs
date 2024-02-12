mod renderer;
use renderer::{resources::{shader::SHADERBUILD_DIR, model::ASSETS_DIR}, *};

use color_eyre::eyre::{eyre, Result};

use crate::window::Window;

pub fn run() -> Result<()> {
    color_eyre::install()?;
    env_logger::init();

    set_directories()?;

    let window = Window::new()?;
    let renderer = Renderer::new(window)?;
    renderer.run_loop()?;

    Ok(())
}

fn set_directories() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 3 {
        return Err(eyre!("Too many args"));
    }
    
    // Set shader build directory from command line args
    if args.len() > 1 {
        unsafe { SHADERBUILD_DIR = Some(args[1].clone()) };

    // Set default shader build directory
    } else {
        let dir = std::env::var("SHADER_BUILD_DIR")
            .unwrap_or_else(|_| "./shaderbuild".to_string());
        unsafe { SHADERBUILD_DIR = Some(dir) };
    }

    // Set assets build directory from command line args
    if args.len() > 2 {
        unsafe { ASSETS_DIR = Some(args[2].clone()) };

    // Set default assets directory
    } else {
        let assets_dir = "./assets";
        unsafe { ASSETS_DIR = Some(assets_dir.into()) };
    }

    Ok(())
}

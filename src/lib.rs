mod renderer;
use std::process::ExitCode;

mod app;

use app::{App, winit_app::WinitApp, egui_app::EguiApp};
use renderer::resources::{model::ASSETS_DIR, shader::SHADERBUILD_DIR};

use color_eyre::eyre::{eyre, Result};

pub fn run() -> Result<ExitCode> {
    color_eyre::install()?;
    env_logger::init();

    set_directories()?;

    //let app = App::<WinitApp>::new()?;
    let app = App::<EguiApp>::new();
    let exit_code = app.run()?;

    Ok(exit_code)
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
        let dir = std::env::var("ASSETS_DIR")
            .unwrap_or_else(|_| "./assets".to_string());
        unsafe { ASSETS_DIR = Some(dir) };
    }

    Ok(())
}


#[allow(unused_imports)]
use color_eyre::eyre::{eyre, Result};
use renderer::{ASSETS_DIR, SHADERBUILD_DIR};
use std::process::ExitCode;

mod egui_app;
mod renderer;

pub fn run() -> Result<ExitCode> {
    color_eyre::install()?;
    env_logger::init();

    set_directories()?;

    let exit_code = egui_app::run()?;

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

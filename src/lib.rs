use bevy::{prelude::*, window::WindowResolution};
use color_eyre::eyre::{eyre, Result};
use renderer::{plugin::RenderPlugin, ASSETS_DIR, SHADERBUILD_DIR};
use std::process::ExitCode;

mod renderer;

pub fn run() -> Result<ExitCode> {
    color_eyre::install()?;

    set_directories()?;

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(1600.0, 900.0),
                    title: "vulkaning".into(),
                    resizable: false,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            RenderPlugin,
        ))
        .run();

    Ok(ExitCode::SUCCESS)
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

use bevy::{
    log,
    prelude::*,
    window::{PrimaryWindow, Window},
    winit::{WinitSettings, WinitWindows},
};
use color_eyre::eyre::{eyre, Result};
use renderer::{ASSETS_DIR, SHADERBUILD_DIR};
use std::process::ExitCode;
use winit::{monitor::VideoMode, window::Fullscreen};

mod egui_app;
mod renderer;

pub fn run() -> Result<ExitCode> {
    color_eyre::install()?;

    set_directories()?;

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(WinitSettings::game())
        .add_systems(Startup, set_up_winit)
        .run();

    //let exit_code = egui_app::run()?;
    //Ok(exit_code)
    Ok(ExitCode::SUCCESS)
}

fn set_up_winit(
    mut winit_windows: NonSendMut<WinitWindows>,
    mut window_ents: Query<Entity, With<PrimaryWindow>>,
) {
    let window_ent = window_ents.single();
    let winit_window = winit_windows.get_window(window_ent).unwrap();
    //winit_window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    log::info!("did not panic!");
    /*
        for (entity, mut window) in &mut windows {
            let winit_window = winit_windows.get_window(entity).unwrap();
        }
    */
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

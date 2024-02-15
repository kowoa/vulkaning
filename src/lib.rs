mod renderer;
use std::{
    process::ExitCode,
    sync::{Arc, Mutex},
};

mod egui_app;

use renderer::{
    resources::{model::ASSETS_DIR, shader::SHADERBUILD_DIR},
    *,
};

use color_eyre::eyre::{eyre, Result};

use crate::window::Window;

pub fn run() -> Result<ExitCode> {
    color_eyre::install()?;
    env_logger::init();

    set_directories()?;

    /*
    let exit_code = egui_ash::run(
        "vulkaning",
        egui_app::EguiAppCreator,
        egui_ash::RunOption {
            viewport_builder: Some(
                egui::ViewportBuilder::default()
                    .with_title("vulkaning")
                    .with_resizable(false)
                    .with_inner_size((800.0, 600.0)),

            ),
            follow_system_theme: false,
            default_theme: egui_ash::Theme::Dark,
            present_mode: ash::vk::PresentModeKHR::FIFO,
            ..Default::default()
        },
    );
    */

    let window = Window::new_without_egui()?;
    let renderer = Renderer::new(&window)?;
    renderer.run_loop_without_egui(window)?;

    //Ok(exit_code)
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


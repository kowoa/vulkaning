use std::{ffi::CString, process::ExitCode};

use ash::vk;
use color_eyre::eyre::{eyre, OptionExt, Result};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

pub struct Window {
    // For use without egui
    pub window: Option<winit::window::Window>,
    pub event_loop: Option<winit::event_loop::EventLoop<()>>,

    // For use with egui
    context: Option<egui::Context>,
    required_instance_extensions: Option<Vec<CString>>,
    required_device_extensions: Option<Vec<CString>>,
    image_registry: Option<egui_ash::ImageRegistry>,
    exit_signal: Option<egui_ash::ExitSignal>,

    width: u32,
    height: u32,
}

impl Window {
    pub fn new_without_egui() -> Result<Self> {
        log::info!("Creating window ...");

        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = winit::window::WindowBuilder::new()
            .with_title("Vulkaning")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600))
            .with_resizable(false)
            .build(&event_loop)?;
        let size = window.inner_size();
        let width = size.width;
        let height = size.height;

        Ok(Self {
            window: Some(window),
            event_loop: Some(event_loop),
            context: None,
            required_instance_extensions: None,
            required_device_extensions: None,
            image_registry: None,
            exit_signal: None,
            width,
            height,
        })
    }

    pub fn new_with_egui(cc: &egui_ash::CreationContext) -> Self {
        log::info!("Creating window ...");
        let size = cc.main_window.inner_size();

        Self {
            window: None,
            event_loop: None,
            context: Some(cc.context.clone()),
            required_instance_extensions: Some(
                cc.required_instance_extensions.clone(),
            ),
            required_device_extensions: Some(
                cc.required_device_extensions.clone(),
            ),
            image_registry: Some(cc.image_registry.clone()),
            exit_signal: Some(cc.exit_signal.clone()),
            width: size.width,
            height: size.height,
        }
    }

    pub fn request_close(&self) -> Result<()> {
        self.exit_signal
            .as_ref()
            .ok_or_eyre("No exit signal found")?
            .send(ExitCode::SUCCESS);

        Ok(())
    }

    pub fn required_instance_extensions(&self) -> Result<Vec<*const i8>> {
        if let Some(event_loop) = &self.event_loop {
            let exts = ash_window::enumerate_required_extensions(
                event_loop.raw_display_handle(),
            )?;
            Ok(exts.to_vec())
        } else if let Some(exts) = &self.required_instance_extensions {
            let exts = exts.iter().map(|ext| ext.as_ptr()).collect::<Vec<_>>();
            // Make sure self.required_instance_extensions lives longer than this returned Vec
            Ok(exts)
        } else {
            Err(eyre!("No required instance extensions found"))
        }
    }

    pub fn required_device_extensions(&self) -> Vec<CString> {
        let mut exts =
            Vec::from([ash::extensions::khr::Swapchain::name().to_owned()]);
        if let Some(e) = &self.required_device_extensions {
            exts.extend(e.clone());
        }
        exts
    }

    pub fn create_surface(
        &self,
        entry: &ash::Entry,
        instance: &ash::Instance,
        winit_window: Option<&winit::window::Window>, // Should be Some if using egui
    ) -> Result<(vk::SurfaceKHR, ash::extensions::khr::Surface)> {
        let window = if let Some(window) = winit_window {
            window
        } else {
            self.window.as_ref().ok_or_eyre("No window found")?
        };

        let surface = unsafe {
            ash_window::create_surface(
                entry,
                instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            )?
        };
        let surface_loader =
            ash::extensions::khr::Surface::new(entry, instance);
        Ok((surface, surface_loader))
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

}

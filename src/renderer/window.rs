use std::{ffi::CString, sync::Arc};

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

        Ok(Self {
            window: Some(window),
            event_loop: Some(event_loop),
            context: None,
            required_instance_extensions: None,
            required_device_extensions: None,
            image_registry: None,
            exit_signal: None,
        })
    }

    pub fn new_with_egui(cc: &egui_ash::CreationContext) -> Self {
        log::info!("Creating window ...");

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
        }
    }

    pub fn required_instance_extensions(&self) -> Result<Vec<CString>> {
        if let Some(event_loop) = &self.event_loop {
            Ok(ash_window::enumerate_required_extensions(
                    event_loop.raw_display_handle(),
                )?
                .iter()
                .map(|ext| unsafe { CString::from_raw(*ext as *mut i8) })
                .collect::<Vec<_>>())
        } else if let Some(exts) = &self.required_instance_extensions {
            Ok(exts.clone())
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
    ) -> Result<(vk::SurfaceKHR, ash::extensions::khr::Surface)> {
        let surface = unsafe {
            let window = self.window.as_ref().ok_or_eyre("No window found")?;
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
}

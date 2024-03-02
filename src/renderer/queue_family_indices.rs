use bevy::log;
use ash::vk;
use color_eyre::eyre::{OptionExt, Result};

pub struct QueueFamilyIndices {
    graphics_family: Option<u32>,
    present_family: Option<u32>,
}

impl QueueFamilyIndices {
    pub fn new(
        instance: &ash::Instance,
        physical_device: &vk::PhysicalDevice,
        surface: &vk::SurfaceKHR,
        surface_loader: &ash::extensions::khr::Surface,
    ) -> Result<Self> {
        let queue_families = unsafe {
            instance
                .get_physical_device_queue_family_properties(*physical_device)
        };

        let mut indices = QueueFamilyIndices {
            graphics_family: None,
            present_family: None,
        };

        for (i, family) in queue_families.iter().enumerate() {
            let i = i as u32;

            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                indices.graphics_family = Some(i);
            }

            let present_support = unsafe {
                surface_loader.get_physical_device_surface_support(
                    *physical_device,
                    i,
                    *surface,
                )?
            };
            if present_support {
                indices.present_family = Some(i);
            }

            if indices.is_complete() {
                break;
            }
        }

        Ok(indices)
    }

    pub fn get_graphics_family(&self) -> Result<u32> {
        self.graphics_family
            .ok_or_eyre("No graphics family index found")
    }

    pub fn get_present_family(&self) -> Result<u32> {
        self.present_family
            .ok_or_eyre("No present family index found")
    }

    pub fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.present_family.is_some()
    }
}

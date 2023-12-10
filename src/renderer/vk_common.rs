use ash::vk;

pub struct SwapchainSupportDetails {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}

pub fn query_swapchain_support(
    device: &vk::PhysicalDevice,
    surface: &vk::SurfaceKHR,
    surface_loader: &ash::extensions::khr::Surface,
) -> anyhow::Result<SwapchainSupportDetails> {
    let capabilities = unsafe {
        surface_loader
            .get_physical_device_surface_capabilities(*device, *surface)?
    };

    let formats = unsafe {
        surface_loader.get_physical_device_surface_formats(*device, *surface)?
    };

    let present_modes = unsafe {
        surface_loader
            .get_physical_device_surface_present_modes(*device, *surface)?
    };

    Ok(SwapchainSupportDetails {
        capabilities,
        formats,
        present_modes,
    })
}

pub struct QueueFamilyIndices {
    pub graphics_family: Option<u32>,
    pub present_family: Option<u32>
}

impl QueueFamilyIndices {
    pub fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.present_family.is_some()
    }
}

pub fn find_queue_families(
    instance: &ash::Instance,
    device: &vk::PhysicalDevice,
    surface: &vk::SurfaceKHR,
    surface_loader: &ash::extensions::khr::Surface,
) -> anyhow::Result<QueueFamilyIndices> {
    let queue_families = unsafe {
        instance.get_physical_device_queue_family_properties(*device)
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
            surface_loader
                .get_physical_device_surface_support(*device, i, *surface)?
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

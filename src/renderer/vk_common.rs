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

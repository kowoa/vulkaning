use ash::vk;

use super::core::Core;

pub struct Swapchain {
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_loader: ash::extensions::khr::Swapchain,
    pub images: Vec<vk::Image>,
    pub image_format: vk::Format,
    pub extent: vk::Extent2D,
    pub image_views: Vec<vk::ImageView>,
}

impl Swapchain {
    pub fn new(
        core: &Core,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> {
        let (
            swapchain,
            swapchain_loader,
            images,
            image_format,
            extent,
        ) = create_swapchain(core, window)?;
        let image_views = create_image_views(
            core,
            &image_format,
            &images,
        )?;

        let objs = Self {
            swapchain,
            swapchain_loader,
            images,
            image_format,
            extent,
            image_views,
        };
        
        Ok(objs)
    }

    pub fn destroy(&self, device: &ash::Device) {
        log::info!("Cleaning up swapchain ...");
        unsafe {
            for view in &self.image_views {
                device.destroy_image_view(*view, None);
            }
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
        }
    }
}

fn create_swapchain(
    core: &Core,
    window: &winit::window::Window,
) -> anyhow::Result<(
    vk::SwapchainKHR,
    ash::extensions::khr::Swapchain,
    Vec<vk::Image>,
    vk::Format,
    vk::Extent2D,
)> {
    let swapchain_support = query_swapchain_support(
        &core.physical_device,
        &core.surface,
        &core.surface_loader,
    )?;

    let surface_format =
        choose_swapchain_surface_format(&swapchain_support.formats);

    let present_mode =
        choose_swapchain_present_mode(&swapchain_support.present_modes);

    let extent =
        choose_swapchain_extent(&swapchain_support.capabilities, window);

    let min_image_count = {
        let min = swapchain_support.capabilities.min_image_count;
        let max = swapchain_support.capabilities.max_image_count;
        // Recommended to request at least one more image than the minimum
        // to prevent having to wait on driver to complete internal operations
        // before another image can be acquired
        if max > 0 && min + 1 > max {
            max
        } else {
            min + 1
        }
    };

    let (image_sharing_mode, queue_family_index_count, queue_family_indices) = {
        let indices = &core.queue_family_indices;
        if indices.graphics_family != indices.present_family {
            (
                // CONCURRENT means images can be used across multiple queue families
                // without explicit ownership transfers
                vk::SharingMode::CONCURRENT,
                2,
                vec![
                    indices.graphics_family.unwrap(),
                    indices.present_family.unwrap(),
                ],
            )
        } else {
            // EXCLUSIVE means image is owned by one queue family at a time
            // and ownership must be explicitly transferred between queue families
            (vk::SharingMode::EXCLUSIVE, 0, Vec::new())
        }
    };

    let info = vk::SwapchainCreateInfoKHR {
        surface: core.surface,
        min_image_count,
        image_format: surface_format.format,
        image_color_space: surface_format.color_space,
        image_extent: extent,
        image_array_layers: 1,
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        image_sharing_mode,
        queue_family_index_count,
        p_queue_family_indices: queue_family_indices.as_ptr(),
        pre_transform: swapchain_support.capabilities.current_transform,
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
        present_mode,
        clipped: vk::TRUE,
        old_swapchain: vk::SwapchainKHR::null(),
        ..Default::default()
    };

    let swapchain_loader = ash::extensions::khr::Swapchain::new(
        &core.instance,
        &core.device,
    );
    let swapchain = unsafe { swapchain_loader.create_swapchain(&info, None)? };
    let swapchain_images =
        unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
    let swapchain_image_format = surface_format.format;
    let swapchain_extent = extent;

    Ok((
        swapchain,
        swapchain_loader,
        swapchain_images,
        swapchain_image_format,
        swapchain_extent,
    ))
}

fn create_image_views(
    core_objs: &Core,
    swapchain_image_format: &vk::Format,
    images: &Vec<vk::Image>,
) -> anyhow::Result<Vec<vk::ImageView>> {
    let views = images
        .iter()
        .map(|image| {
            let info = vk::ImageViewCreateInfo {
                image: *image,
                view_type: vk::ImageViewType::TYPE_2D,
                format: *swapchain_image_format,
                components: vk::ComponentMapping {
                    r: vk::ComponentSwizzle::IDENTITY,
                    g: vk::ComponentSwizzle::IDENTITY,
                    b: vk::ComponentSwizzle::IDENTITY,
                    a: vk::ComponentSwizzle::IDENTITY,
                },
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                ..Default::default()
            };

            unsafe { core_objs.device.create_image_view(&info, None) }
        })
        .collect::<ash::prelude::VkResult<Vec<_>>>()?;

    Ok(views)
}

fn choose_swapchain_surface_format(
    available_formats: &Vec<vk::SurfaceFormatKHR>,
) -> vk::SurfaceFormatKHR {
    let format = available_formats.iter().find(|format| {
        format.format == vk::Format::B8G8R8A8_SRGB
            && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    });

    *format.unwrap()
}

fn choose_swapchain_present_mode(
    available_present_modes: &Vec<vk::PresentModeKHR>,
) -> vk::PresentModeKHR {
    let mode = available_present_modes
        .iter()
        .find(|mode| **mode == vk::PresentModeKHR::FIFO_RELAXED);

    match mode {
        Some(mode) => *mode,
        None => vk::PresentModeKHR::FIFO,
    }
}

fn choose_swapchain_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    window: &winit::window::Window,
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        let win_sz = window.inner_size();
        vk::Extent2D {
            width: num::clamp(
                win_sz.width,
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ),
            height: num::clamp(
                win_sz.height,
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ),
        }
    }
}

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
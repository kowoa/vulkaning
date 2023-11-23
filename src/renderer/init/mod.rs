use std::{collections::HashSet, ffi::{CStr, c_char, CString, c_void}};

use anyhow::anyhow;
use ash::vk;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{event_loop::EventLoop};

mod utils;
use utils::c_char_to_string;

mod create_info;
use create_info::*;

const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
const REQUIRED_VALIDATION_LAYERS: [&'static str; 1] = [
    "VK_LAYER_KHRONOS_validation",
];
const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 1] = [
    ash::extensions::khr::Swapchain::name()
];

pub struct VulkanCore {
    entry: ash::Entry,
    instance: ash::Instance,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    debug_messenger_loader: ash::extensions::ext::DebugUtils,
    surface: vk::SurfaceKHR,
    surface_loader: ash::extensions::khr::Surface,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
}

struct QueueFamilyIndices {
    graphics_family: Option<u32>,
    present_family: Option<u32>
}

impl QueueFamilyIndices {
    pub fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.present_family.is_some()
    }
}

impl VulkanCore {
    pub fn new(
        window: &winit::window::Window,
        event_loop: &EventLoop<()>,
    ) -> anyhow::Result<Self> {
        let entry = ash::Entry::linked();
        let instance = Self::create_instance(&entry, event_loop)?;
        let (debug_messenger, debug_messenger_loader) =
            Self::create_debug_messenger(&entry, &instance)?;
        let (surface, surface_loader) =
            Self::create_surface(&entry, &instance, window)?;
        let physical_device =
            Self::create_physical_device(&instance, &surface, &surface_loader)?;

        Ok(Self {
            entry,
            instance,
            debug_messenger,
            debug_messenger_loader,
            surface,
            surface_loader,
            physical_device,
        })
    }

    pub fn destroy(&mut self,
        allocation_callbacks: Option<&vk::AllocationCallbacks>
    ) {
        unsafe {
            self.instance.destroy_instance(allocation_callbacks);
        }
    }

    fn create_instance(
        entry: &ash::Entry,
        event_loop: &EventLoop<()>
    ) -> anyhow::Result<ash::Instance> {
        if ENABLE_VALIDATION_LAYERS {
            check_required_validation_layers(entry)?;
        }

        let app_info = vk::ApplicationInfo::default();

        let req_ext_names = get_required_extension_names(event_loop)?;
        let req_layer_names_cstring = REQUIRED_VALIDATION_LAYERS
            .iter()
            .map(|&s| CString::new(s))
            .collect::<Result<Vec<_>, _>>()?;
        let req_layer_names_cstr = req_layer_names_cstring
            .iter()
            .map(|s| s.as_c_str())
            .collect::<Vec<_>>();

        let debug_info = debug_utils_messenger_create_info();
        let instance_info = vk::InstanceCreateInfo {
            p_next: if ENABLE_VALIDATION_LAYERS {
                &debug_info
                    as *const vk::DebugUtilsMessengerCreateInfoEXT
                    as *const c_void
            } else { std::ptr::null() },
            p_application_info: &app_info,
            enabled_layer_count: if ENABLE_VALIDATION_LAYERS {
                req_layer_names_cstr.len() as u32
            } else { 0 },
            pp_enabled_layer_names: if ENABLE_VALIDATION_LAYERS {
                req_layer_names_cstr.as_ptr() as *const *const i8
            } else { std::ptr::null() },
            enabled_extension_count: req_ext_names.len() as u32,
            pp_enabled_extension_names: req_ext_names.as_ptr(),
            ..Default::default()
        };

        Ok(unsafe { entry.create_instance(&instance_info, None)? })
    }

    fn create_debug_messenger(
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> anyhow::Result<(
        vk::DebugUtilsMessengerEXT,
        ash::extensions::ext::DebugUtils,
    )> {
        let debug_messenger_loader =
            ash::extensions::ext::DebugUtils::new(entry, instance);

        if ENABLE_VALIDATION_LAYERS {
            let info = debug_utils_messenger_create_info();
            let debug_messenger = unsafe {
                debug_messenger_loader
                    .create_debug_utils_messenger(&info, None)?
            };
            Ok((debug_messenger, debug_messenger_loader))
        } else {
            Ok((vk::DebugUtilsMessengerEXT::null(), debug_messenger_loader))
        }
    }

    fn create_surface(
        entry: &ash::Entry,
        instance: &ash::Instance,
        window: &winit::window::Window,
    ) -> anyhow::Result<(vk::SurfaceKHR, ash::extensions::khr::Surface)> {
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

    fn create_physical_device(
        instance: &ash::Instance,
        surface: &vk::SurfaceKHR,
        surface_loader: &ash::extensions::khr::Surface,
    ) -> anyhow::Result<vk::PhysicalDevice> {
        let devices = unsafe { instance.enumerate_physical_devices()? };
        if devices.is_empty() {
            return Err(anyhow!("Failed to find a GPU with Vulkan support"));
        }

        let suitable_devices = devices
            .iter()
            .filter(|device| {
                physical_device_is_suitable(
                    device,
                    instance,
                    surface,
                    surface_loader,
                )
                .is_ok_and(|suitable| suitable)
            })
            .collect::<Vec<_>>();

        let chosen_device = suitable_devices.get(0);
        match chosen_device {
            Some(device) => Ok(**device),
            None => Err(anyhow!("Failed to find a suitable GPU")),
        }
    }
}

fn check_required_validation_layers(
    entry: &ash::Entry
) -> anyhow::Result<()> {
    if !ENABLE_VALIDATION_LAYERS { return Ok(()); }

    let available_layers = entry
        .enumerate_instance_layer_properties()?
        .iter()
        .map(|props| {
            c_char_to_string(&props.layer_name)
        })
        .collect::<Result<HashSet<_>, _>>()?;

    let all_layers_found = REQUIRED_VALIDATION_LAYERS
        .iter()
        .all(|layer| available_layers.contains(*layer));

    match all_layers_found {
        true => Ok(()),
        false => Err(anyhow!("Required validation layers are not all available"))
    }
}

fn get_required_extension_names(
    event_loop: &EventLoop<()>
) -> anyhow::Result<Vec<*const i8>> {
    let mut ext_names = Vec::new();
    ext_names.extend(ash_window::enumerate_required_extensions(
        event_loop.raw_display_handle(),
    )?);
    if ENABLE_VALIDATION_LAYERS {
        ext_names.extend(
            [ash::extensions::ext::DebugUtils::name().as_ptr()]
        );
    }
    Ok(ext_names)
}

fn physical_device_is_suitable(
    device: &vk::PhysicalDevice,
    instance: &ash::Instance,
    surface: &vk::SurfaceKHR,
    surface_loader: &ash::extensions::khr::Surface,
) -> anyhow::Result<bool> {

    #[cfg(debug_assertions)]
    {
        let dev_properties =
            unsafe { instance.get_physical_device_properties(*device) };
        let dev_features =
            unsafe { instance.get_physical_device_features(*device) };
        let dev_queue_families = unsafe {
            instance.get_physical_device_queue_family_properties(*device)
        };
        let dev_type = match dev_properties.device_type {
            vk::PhysicalDeviceType::CPU => "CPU",
            vk::PhysicalDeviceType::INTEGRATED_GPU => "Integrated GPU",
            vk::PhysicalDeviceType::DISCRETE_GPU => "Discrete GPU",
            vk::PhysicalDeviceType::VIRTUAL_GPU => "Virtual GPU",
            vk::PhysicalDeviceType::OTHER => "Unknown",
            _ => panic!("Unknown device type"),
        };
        let dev_name = utils::c_char_to_string(&dev_properties.device_name)?;
        println!(
            "\tDevice name: {}, ID: {}, Type: {}",
            dev_name, dev_properties.device_id, dev_type
        );

        println!(
            "\tAPI version: {}.{}.{}",
            vk::api_version_major(dev_properties.api_version),
            vk::api_version_minor(dev_properties.api_version),
            vk::api_version_patch(dev_properties.api_version),
        );

        println!("\tSupported queue families: {}", dev_queue_families.len());
        println!(
            "\t\tQueue Count | Graphics, Compute, Transfer, Sparse Binding"
        );
        let b2s = |b: bool| if b { "YES" } else { "NO" };
        for queue_family in dev_queue_families {
            let flags = queue_family.queue_flags;
            let graphics = b2s(flags.contains(vk::QueueFlags::GRAPHICS));
            let compute = b2s(flags.contains(vk::QueueFlags::COMPUTE));
            let transfer = b2s(flags.contains(vk::QueueFlags::TRANSFER));
            let sparse = b2s(flags.contains(vk::QueueFlags::SPARSE_BINDING));
            println!(
                "\t\t{} | {}, {}, {}, {}",
                queue_family.queue_count, graphics, compute, transfer, sparse,
            );
        }
        println!(
            "\tGeometry shader support: {}",
            b2s(dev_features.geometry_shader == 1)
        );
    }

    let indices =
        find_queue_families(device, instance, surface, surface_loader)?;

    let exts_supported = check_required_device_extensions(device, instance)?;

    let swapchain_adequate = {
        let details = query_swapchain_support(device, surface, surface_loader)?;
        !details.formats.is_empty() && !details.present_modes.is_empty()
    };

    Ok(indices.is_complete() && exts_supported && swapchain_adequate)
}

fn find_queue_families(
    device: &vk::PhysicalDevice,
    instance: &ash::Instance,
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

fn check_required_device_extensions(
    device: &vk::PhysicalDevice,
    instance: &ash::Instance,
) -> anyhow::Result<bool> {
    let available_exts =
        unsafe { instance.enumerate_device_extension_properties(*device)? }
            .iter()
            .map(|ext| c_char_to_string(&ext.extension_name))
            .collect::<Vec<_>>();

    Ok(REQUIRED_DEVICE_EXTENSIONS
        .iter()
        .all(|ext| available_exts.contains(&ext.to_str().unwrap().to_string())))
}

fn query_swapchain_support(
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
        .find(|mode| **mode == vk::PresentModeKHR::MAILBOX);

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

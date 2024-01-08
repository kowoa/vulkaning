// Engine initialization

use std::{
    collections::HashSet,
    ffi::{c_void, CStr, CString},
};

use anyhow::anyhow;
use ash::vk;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::event_loop::EventLoop;

use crate::renderer::vk_initializers;

use super::{destruction_queue::Destroy, swapchain::query_swapchain_support, utils};

const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
const REQUIRED_VALIDATION_LAYERS: [&'static str; 1] =
    ["VK_LAYER_KHRONOS_validation"];
const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 1] =
    [ash::extensions::khr::Swapchain::name()];

pub struct Core {
    _entry: ash::Entry,

    pub instance: ash::Instance,

    pub debug_messenger: vk::DebugUtilsMessengerEXT,
    pub debug_messenger_loader: ash::extensions::ext::DebugUtils,

    pub surface: vk::SurfaceKHR,
    pub surface_loader: ash::extensions::khr::Surface,

    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,

    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub queue_family_indices: QueueFamilyIndices,

    pub allocator: Allocator,
}

impl Core {
    pub fn new(
        window: &winit::window::Window,
        event_loop: &EventLoop<()>,
    ) -> anyhow::Result<Self> {
        let entry = ash::Entry::linked();
        let instance = create_instance(&entry, event_loop)?;
        let (debug_messenger, debug_messenger_loader) =
            create_debug_messenger(&entry, &instance)?;
        let (surface, surface_loader) =
            create_surface(&entry, &instance, window)?;
        let physical_device =
            create_physical_device(&instance, &surface, &surface_loader)?;
        let (device, graphics_queue, present_queue, queue_family_indices) =
            create_logical_device(
                &instance,
                &physical_device,
                &surface,
                &surface_loader,
            )?;
        
        let allocator = {
            let info = AllocatorCreateDesc {
                instance: instance.clone(),
                device: device.clone(),
                physical_device,
                debug_settings: Default::default(),
                buffer_device_address: true,
                allocation_sizes: Default::default(),
            };
            Allocator::new(&info)?
        };

        Ok(Self {
            _entry: entry,
            instance,
            debug_messenger,
            debug_messenger_loader,
            surface,
            surface_loader,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            queue_family_indices,
            allocator
        })
    }
}

impl Destroy for Core {
    fn destroy(&self, device: &ash::Device) {
        log::info!("Cleaning up core ...");
        unsafe {
            self.device.destroy_device(None);
            // Segfault occurs here if window gets destroyed before surface
            self.surface_loader.destroy_surface(self.surface, None);
            if ENABLE_VALIDATION_LAYERS {
                self.debug_messenger_loader
                    .destroy_debug_utils_messenger(self.debug_messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

fn create_instance(
    entry: &ash::Entry,
    event_loop: &EventLoop<()>,
) -> anyhow::Result<ash::Instance> {
    if ENABLE_VALIDATION_LAYERS {
        check_required_validation_layers(entry)?;
    }

    let app_info = vk::ApplicationInfo {
        api_version: vk::API_VERSION_1_3,
        ..Default::default()
    };

    let req_ext_names = get_required_extension_names(event_loop)?;
    let req_layer_names_cstring = REQUIRED_VALIDATION_LAYERS
        .iter()
        .map(|&s| CString::new(s))
        .collect::<Result<Vec<_>, _>>()?;
    let req_layer_names_cstr = req_layer_names_cstring
        .iter()
        .map(|s| s.as_c_str())
        .collect::<Vec<_>>();

    let debug_info = vk_initializers::debug_utils_messenger_create_info();
    let instance_info = vk::InstanceCreateInfo {
        p_next: if ENABLE_VALIDATION_LAYERS {
            &debug_info as *const vk::DebugUtilsMessengerCreateInfoEXT
                as *const c_void
        } else {
            std::ptr::null()
        },
        p_application_info: &app_info,
        enabled_layer_count: if ENABLE_VALIDATION_LAYERS {
            req_layer_names_cstr.len() as u32
        } else {
            0
        },
        pp_enabled_layer_names: if ENABLE_VALIDATION_LAYERS {
            req_layer_names_cstr.as_ptr() as *const *const i8
        } else {
            std::ptr::null()
        },
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
        let info = vk_initializers::debug_utils_messenger_create_info();
        let debug_messenger = unsafe {
            debug_messenger_loader.create_debug_utils_messenger(&info, None)?
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
    let surface_loader = ash::extensions::khr::Surface::new(entry, instance);
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

fn create_logical_device(
    instance: &ash::Instance,
    physical_device: &vk::PhysicalDevice,
    surface: &vk::SurfaceKHR,
    surface_loader: &ash::extensions::khr::Surface,
) -> anyhow::Result<(ash::Device, vk::Queue, vk::Queue, QueueFamilyIndices)> {
    let indices = find_queue_families(
        instance,
        physical_device,
        surface,
        surface_loader,
    )?;

    let graphics_family = indices
        .graphics_family
        .ok_or(anyhow!("Graphics queue family not initialized"))?;
    let present_family = indices
        .present_family
        .ok_or(anyhow!("Presentation queue family not initialized"))?;
    let unique_queue_families =
        HashSet::from([graphics_family, present_family]);

    let queue_priorities = [1.0f32];
    let queue_infos = unique_queue_families
        .iter()
        .map(|family| vk::DeviceQueueCreateInfo {
            queue_family_index: *family,
            p_queue_priorities: queue_priorities.as_ptr(),
            queue_count: queue_priorities.len() as u32,
            ..Default::default()
        })
        .collect::<Vec<_>>();

    let physical_device_features = vk::PhysicalDeviceFeatures::default();

    let req_ext_names = REQUIRED_DEVICE_EXTENSIONS
        .iter()
        .map(|ext| ext.as_ptr())
        .collect::<Vec<_>>();
    let device_info = vk::DeviceCreateInfo {
        p_queue_create_infos: queue_infos.as_ptr(),
        p_enabled_features: &physical_device_features,
        queue_create_info_count: queue_infos.len() as u32,
        enabled_extension_count: req_ext_names.len() as u32,
        pp_enabled_extension_names: req_ext_names.as_ptr(),
        ..Default::default()
    };

    let device = unsafe {
        instance.create_device(*physical_device, &device_info, None)?
    };

    let graphics_queue = unsafe { device.get_device_queue(graphics_family, 0) };
    let present_queue = unsafe { device.get_device_queue(present_family, 0) };

    Ok((device, graphics_queue, present_queue, indices))
}

fn check_required_validation_layers(entry: &ash::Entry) -> anyhow::Result<()> {
    if !ENABLE_VALIDATION_LAYERS {
        return Ok(());
    }

    let available_layers = entry
        .enumerate_instance_layer_properties()?
        .iter()
        .map(|props| utils::c_char_to_string(&props.layer_name))
        .collect::<Result<HashSet<_>, _>>()?;

    let all_layers_found = REQUIRED_VALIDATION_LAYERS
        .iter()
        .all(|layer| available_layers.contains(*layer));

    match all_layers_found {
        true => Ok(()),
        false => {
            Err(anyhow!("Required validation layers are not all available"))
        }
    }
}

fn get_required_extension_names(
    event_loop: &EventLoop<()>,
) -> anyhow::Result<Vec<*const i8>> {
    let mut ext_names = Vec::new();
    ext_names.extend(ash_window::enumerate_required_extensions(
        event_loop.raw_display_handle(),
    )?);
    if ENABLE_VALIDATION_LAYERS {
        ext_names.extend([ash::extensions::ext::DebugUtils::name().as_ptr()]);
    }
    Ok(ext_names)
}

fn physical_device_is_suitable(
    physical_device: &vk::PhysicalDevice,
    instance: &ash::Instance,
    surface: &vk::SurfaceKHR,
    surface_loader: &ash::extensions::khr::Surface,
) -> anyhow::Result<bool> {
    #[cfg(debug_assertions)]
    {
        let dev_properties = unsafe {
            instance.get_physical_device_properties(*physical_device)
        };
        let dev_features =
            unsafe { instance.get_physical_device_features(*physical_device) };
        let dev_queue_families = unsafe {
            instance
                .get_physical_device_queue_family_properties(*physical_device)
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

    let indices = find_queue_families(
        instance,
        physical_device,
        surface,
        surface_loader,
    )?;

    let exts_supported =
        check_required_device_extensions(physical_device, instance)?;

    let swapchain_adequate = {
        let details = query_swapchain_support(
            physical_device,
            surface,
            surface_loader,
        )?;
        !details.formats.is_empty() && !details.present_modes.is_empty()
    };

    Ok(indices.is_complete() && exts_supported && swapchain_adequate)
}

fn check_required_device_extensions(
    physical_device: &vk::PhysicalDevice,
    instance: &ash::Instance,
) -> anyhow::Result<bool> {
    let available_exts = unsafe {
        instance.enumerate_device_extension_properties(*physical_device)?
    }
    .iter()
    .map(|ext| utils::c_char_to_string(&ext.extension_name))
    .collect::<Result<Vec<_>, _>>()?;

    Ok(REQUIRED_DEVICE_EXTENSIONS
        .iter()
        .map(|ext| ext.to_str())
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .all(|ext| available_exts.contains(&ext.to_string())))
}

pub struct QueueFamilyIndices {
    pub graphics_family: Option<u32>,
    pub present_family: Option<u32>,
}

impl QueueFamilyIndices {
    pub fn is_complete(&self) -> bool {
        self.graphics_family.is_some() && self.present_family.is_some()
    }
}

fn find_queue_families(
    instance: &ash::Instance,
    physical_device: &vk::PhysicalDevice,
    surface: &vk::SurfaceKHR,
    surface_loader: &ash::extensions::khr::Surface,
) -> anyhow::Result<QueueFamilyIndices> {
    let queue_families = unsafe {
        instance.get_physical_device_queue_family_properties(*physical_device)
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

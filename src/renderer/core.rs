use bevy::log;
use color_eyre::eyre::{eyre, Result};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::{
    collections::HashSet,
    ffi::{c_void, CStr, CString},
    mem::ManuallyDrop,
    sync::{Arc, Mutex, MutexGuard},
};

use ash::vk;
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};

use super::{
    queue_family_indices::QueueFamilyIndices,
    swapchain::query_swapchain_support, vkinit, vkutils,
};

pub struct Core {
    pub entry: ash::Entry,

    pub instance: ash::Instance,

    pub debug_messenger: vk::DebugUtilsMessengerEXT,
    pub debug_messenger_loader: ash::extensions::ext::DebugUtils,

    pub surface: vk::SurfaceKHR,
    pub surface_loader: ash::extensions::khr::Surface,

    pub physical_device: vk::PhysicalDevice,
    pub physical_device_props: vk::PhysicalDeviceProperties,
    pub device: ash::Device,

    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub queue_family_indices: QueueFamilyIndices,

    allocator: ManuallyDrop<Arc<Mutex<Allocator>>>,
}

impl Core {
    const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
    const REQUIRED_VALIDATION_LAYERS: [&'static str; 1] =
        ["VK_LAYER_KHRONOS_validation"];

    pub fn new(window: &winit::window::Window) -> Result<Self> {
        let req_instance_exts = Self::get_required_instance_extensions(window)?;

        println!("{:#?}", req_instance_exts);

        let req_device_exts = Self::get_required_device_extensions();

        println!("{:#?}", req_device_exts);

        let entry = ash::Entry::linked();
        let instance = Self::create_instance(&entry, &req_instance_exts)?;
        let (debug_messenger, debug_messenger_loader) =
            Self::create_debug_messenger(&entry, &instance)?;
        let (surface, surface_loader) =
            Self::create_surface(&entry, &instance, window)?;
        let physical_device = Self::create_physical_device(
            &instance,
            &surface,
            &surface_loader,
            &req_device_exts,
        )?;

        let physical_device_props =
            unsafe { instance.get_physical_device_properties(physical_device) };
        log::info!(
            "GPU has a minimum buffer alignment of {}",
            physical_device_props
                .limits
                .min_uniform_buffer_offset_alignment
        );

        let (device, graphics_queue, present_queue, queue_family_indices) =
            Self::create_logical_device(
                &instance,
                &physical_device,
                &surface,
                &surface_loader,
                &req_device_exts,
            )?;

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device,
            debug_settings: AllocatorDebugSettings {
                log_memory_information: true,
                log_leaks_on_shutdown: true,
                store_stack_traces: false,
                log_allocations: true,
                log_frees: true,
                log_stack_traces: false,
            },
            buffer_device_address: true,
            allocation_sizes: Default::default(),
        })?;

        Ok(Self {
            entry,
            instance,
            debug_messenger,
            debug_messenger_loader,
            surface,
            surface_loader,
            physical_device,
            physical_device_props,
            device,
            graphics_queue,
            present_queue,
            queue_family_indices,
            allocator: ManuallyDrop::new(Arc::new(Mutex::new(allocator))),
        })
    }

    pub fn cleanup(mut self) {
        log::info!("Cleaning up core ...");
        unsafe {
            // We need to do this because the allocator doesn't destroy all
            // memory blocks (VkDeviceMemory) until it is dropped.
            ManuallyDrop::drop(&mut self.allocator);

            self.device.destroy_device(None);
            // Segfault occurs here if window gets destroyed before surface
            self.surface_loader.destroy_surface(self.surface, None);
            if Self::ENABLE_VALIDATION_LAYERS {
                self.debug_messenger_loader
                    .destroy_debug_utils_messenger(self.debug_messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }

    pub fn get_allocator(&self) -> Result<MutexGuard<Allocator>> {
        match self.allocator.lock() {
            Ok(allocator) => Ok(allocator),
            Err(err) => Err(eyre!(err.to_string())),
        }
    }

    pub fn get_allocator_ref(&self) -> Arc<Mutex<Allocator>> {
        Arc::clone(&self.allocator)
    }

    pub fn min_uniform_buffer_offset_alignment(&self) -> u64 {
        self.physical_device_props
            .limits
            .min_uniform_buffer_offset_alignment
    }

    /// Returns the padded size of the buffer according to the min alignment
    pub fn pad_uniform_buffer_size(&self, original_size: u64) -> u64 {
        vkutils::pad_uniform_buffer_size(
            original_size,
            self.min_uniform_buffer_offset_alignment(),
        )
    }

    fn get_required_instance_extensions(
        window: &winit::window::Window,
    ) -> Result<Vec<CString>> {
        let mut exts = Vec::new();
        let window_exts = ash_window::enumerate_required_extensions(
            window.raw_display_handle(),
        )?
        .iter()
        .map(|ext| unsafe { CStr::from_ptr(*ext).to_owned() })
        .collect::<Vec<_>>();
        exts.extend(window_exts);
        if Self::ENABLE_VALIDATION_LAYERS {
            exts.push(ash::extensions::ext::DebugUtils::name().to_owned());
        }
        #[cfg(target_os = "macos")]
        exts.push(vk::KhrGetPhysicalDeviceProperties2Fn::name().to_owned());
        Ok(exts)
    }

    fn get_required_device_extensions() -> Vec<CString> {
        #[allow(unused_mut)]
        let mut exts = vec![
            ash::extensions::khr::Swapchain::name().to_owned(),
            ash::extensions::khr::DynamicRendering::name().to_owned(),
        ];
        #[cfg(target_os = "macos")]
        exts.push(vk::KhrPortabilitySubsetFn::name().to_owned());
        exts
    }

    fn create_instance(
        entry: &ash::Entry,
        req_instance_exts: &[CString],
    ) -> Result<ash::Instance> {
        if Self::ENABLE_VALIDATION_LAYERS {
            Self::check_required_validation_layers(entry)?;
        }

        let app_info = vk::ApplicationInfo {
            api_version: vk::API_VERSION_1_3,
            ..Default::default()
        };

        let req_layers = Self::REQUIRED_VALIDATION_LAYERS
            .iter()
            .map(|&s| CString::new(s))
            .collect::<Result<Vec<_>, _>>()?;
        let req_layers_ptr =
            req_layers.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();

        let req_instance_exts = req_instance_exts
            .iter()
            .map(|ext| ext.as_ptr())
            .collect::<Vec<_>>();

        let debug_info = vkinit::debug_utils_messenger_create_info();
        let instance_info = vk::InstanceCreateInfo {
            p_next: if Self::ENABLE_VALIDATION_LAYERS {
                &debug_info as *const vk::DebugUtilsMessengerCreateInfoEXT
                    as *const c_void
            } else {
                std::ptr::null()
            },
            p_application_info: &app_info,
            enabled_layer_count: if Self::ENABLE_VALIDATION_LAYERS {
                req_layers.len() as u32
            } else {
                0
            },
            pp_enabled_layer_names: if Self::ENABLE_VALIDATION_LAYERS {
                req_layers_ptr.as_ptr()
            } else {
                std::ptr::null()
            },
            enabled_extension_count: req_instance_exts.len() as u32,
            pp_enabled_extension_names: req_instance_exts.as_ptr(),
            ..Default::default()
        };

        Ok(unsafe { entry.create_instance(&instance_info, None)? })
    }

    fn create_debug_messenger(
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Result<(vk::DebugUtilsMessengerEXT, ash::extensions::ext::DebugUtils)>
    {
        let debug_messenger_loader =
            ash::extensions::ext::DebugUtils::new(entry, instance);

        if Self::ENABLE_VALIDATION_LAYERS {
            let info = vkinit::debug_utils_messenger_create_info();
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
    ) -> Result<(vk::SurfaceKHR, ash::extensions::khr::Surface)> {
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
        req_device_exts: &Vec<CString>,
    ) -> Result<vk::PhysicalDevice> {
        let devices = unsafe { instance.enumerate_physical_devices()? };
        if devices.is_empty() {
            return Err(eyre!("Failed to find a GPU with Vulkan support"));
        }

        let suitable_devices = devices
            .iter()
            .filter(|device| {
                Self::physical_device_is_suitable(
                    device,
                    req_device_exts,
                    instance,
                    surface,
                    surface_loader,
                )
                .is_ok_and(|suitable| suitable)
            })
            .collect::<Vec<_>>();

        let chosen_device = suitable_devices.first();
        match chosen_device {
            Some(device) => {
                Self::log_physical_device_info(device, instance)?;
                Ok(**device)
            }
            None => Err(eyre!("Failed to find a suitable GPU")),
        }
    }

    fn create_logical_device(
        instance: &ash::Instance,
        physical_device: &vk::PhysicalDevice,
        surface: &vk::SurfaceKHR,
        surface_loader: &ash::extensions::khr::Surface,
        req_device_exts: &Vec<CString>,
    ) -> Result<(ash::Device, vk::Queue, vk::Queue, QueueFamilyIndices)> {
        let indices = QueueFamilyIndices::new(
            instance,
            physical_device,
            surface,
            surface_loader,
        )?;

        let graphics_family = indices.get_graphics_family()?;
        let present_family = indices.get_present_family()?;
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
        let req_device_exts = req_device_exts
            .iter()
            .map(|ext| ext.as_ptr())
            .collect::<Vec<_>>();

        // Enable dynamic rendering
        let dyn_rendering_feats =
            [vk::PhysicalDeviceDynamicRenderingFeaturesKHR::builder()
                .dynamic_rendering(true)
                .build()];
        // Enable synchronization2 feature
        let sync2_feats = [vk::PhysicalDeviceSynchronization2Features {
            synchronization2: vk::TRUE,
            p_next: dyn_rendering_feats.as_ptr() as *mut c_void,
            ..Default::default()
        }];
        // Enable buffer device address
        let mut buffer_device_address_features =
            vk::PhysicalDeviceBufferDeviceAddressFeatures {
                buffer_device_address: vk::TRUE,
                p_next: sync2_feats.as_ptr() as *mut c_void,
                ..Default::default()
            };
        let shader_draw_params_features =
            vk::PhysicalDeviceShaderDrawParametersFeatures {
                shader_draw_parameters: vk::TRUE,
                p_next: &mut buffer_device_address_features
                    as *mut vk::PhysicalDeviceBufferDeviceAddressFeatures
                    as *mut c_void,
                ..Default::default()
            };
        let device_info = vk::DeviceCreateInfo {
            p_queue_create_infos: queue_infos.as_ptr(),
            p_enabled_features: &physical_device_features,
            queue_create_info_count: queue_infos.len() as u32,
            enabled_extension_count: req_device_exts.len() as u32,
            pp_enabled_extension_names: req_device_exts.as_ptr(),
            p_next: &shader_draw_params_features
                as *const vk::PhysicalDeviceShaderDrawParametersFeatures
                as *const c_void,
            ..Default::default()
        };

        let device = unsafe {
            instance.create_device(*physical_device, &device_info, None)?
        };

        let graphics_queue =
            unsafe { device.get_device_queue(graphics_family, 0) };
        let present_queue =
            unsafe { device.get_device_queue(present_family, 0) };

        Ok((device, graphics_queue, present_queue, indices))
    }

    fn check_required_validation_layers(entry: &ash::Entry) -> Result<()> {
        if !Self::ENABLE_VALIDATION_LAYERS {
            return Ok(());
        }

        let available_layers = entry
            .enumerate_instance_layer_properties()?
            .iter()
            .map(|props| vkutils::c_char_to_string(&props.layer_name))
            .collect::<Result<HashSet<_>, _>>()?;

        let all_layers_found = Self::REQUIRED_VALIDATION_LAYERS
            .iter()
            .all(|layer| available_layers.contains(*layer));

        match all_layers_found {
            true => Ok(()),
            false => {
                Err(eyre!("Required validation layers are not all available"))
            }
        }
    }

    fn physical_device_is_suitable(
        physical_device: &vk::PhysicalDevice,
        req_device_exts: &Vec<CString>,
        instance: &ash::Instance,
        surface: &vk::SurfaceKHR,
        surface_loader: &ash::extensions::khr::Surface,
    ) -> Result<bool> {
        let indices = QueueFamilyIndices::new(
            instance,
            physical_device,
            surface,
            surface_loader,
        )?;

        let exts_supported = Self::physical_device_has_extensions(
            physical_device,
            req_device_exts,
            instance,
        )?;

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

    fn log_physical_device_info(
        physical_device: &vk::PhysicalDevice,
        instance: &ash::Instance,
    ) -> Result<()> {
        let mut message = String::new();
        message.push_str("\nPhysical Device Info:\n");

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
            vk::PhysicalDeviceType::CPU => Ok("CPU"),
            vk::PhysicalDeviceType::INTEGRATED_GPU => Ok("Integrated GPU"),
            vk::PhysicalDeviceType::DISCRETE_GPU => Ok("Discrete GPU"),
            vk::PhysicalDeviceType::VIRTUAL_GPU => Ok("Virtual GPU"),
            vk::PhysicalDeviceType::OTHER => Ok("Unknown"),
            _ => Err(eyre!("Unknown device type")),
        }?;
        let dev_name = vkutils::c_char_to_string(&dev_properties.device_name)?;
        message.push_str(&format!(
            "\tDevice name: {}, ID: {}, Type: {}\n",
            dev_name, dev_properties.device_id, dev_type
        ));

        message.push_str(&format!(
            "\tSupported API version: {}.{}.{}\n",
            vk::api_version_major(dev_properties.api_version),
            vk::api_version_minor(dev_properties.api_version),
            vk::api_version_patch(dev_properties.api_version),
        ));

        message.push_str(&format!(
            "\tSupported queue families: {}\n",
            dev_queue_families.len()
        ));
        message.push_str(
            "\t\tQueue Count | Graphics, Compute, Transfer, Sparse Binding\n",
        );

        let b2s = |b: bool| if b { "YES" } else { " NO" };
        for queue_family in dev_queue_families {
            let flags = queue_family.queue_flags;
            let graphics = b2s(flags.contains(vk::QueueFlags::GRAPHICS));
            let compute = b2s(flags.contains(vk::QueueFlags::COMPUTE));
            let transfer = b2s(flags.contains(vk::QueueFlags::TRANSFER));
            let sparse = b2s(flags.contains(vk::QueueFlags::SPARSE_BINDING));
            message.push_str(&format!(
                "\t\t{} | {}, {}, {}, {}\n",
                queue_family.queue_count, graphics, compute, transfer, sparse,
            ));
        }
        message.push_str(&format!(
            "\tGeometry shader support: {}\n",
            b2s(dev_features.geometry_shader == 1)
        ));

        log::info!("{}", message);

        Ok(())
    }

    /// Check if the physical device has all the required device extensions
    fn physical_device_has_extensions(
        physical_device: &vk::PhysicalDevice,
        req_device_exts: &Vec<CString>,
        instance: &ash::Instance,
    ) -> Result<bool> {
        let available_exts = unsafe {
            instance.enumerate_device_extension_properties(*physical_device)?
        }
        .iter()
        .map(|ext| vkutils::c_char_to_cstring(&ext.extension_name))
        .collect::<Vec<_>>();

        let contains_all = req_device_exts
            .iter()
            .all(|ext| available_exts.contains(ext));

        Ok(contains_all)
    }
}

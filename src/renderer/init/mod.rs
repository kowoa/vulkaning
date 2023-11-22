use std::{collections::HashSet, ffi::{CStr, c_char, CString, c_void}};

use anyhow::anyhow;
use ash::vk;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{event_loop::EventLoop};

mod utils;
use utils::*;

mod create_info;
use create_info::*;

const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
const REQUIRED_VALIDATION_LAYERS: [&'static str; 1] = [
    "VK_LAYER_KHRONOS_validation",
];

pub struct VulkanCore {
    entry: ash::Entry,
    instance: ash::Instance,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    debug_messenger_loader: ash::extensions::ext::DebugUtils,
}

impl VulkanCore {
    pub fn new(event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
        let entry = ash::Entry::linked();
        let instance = Self::create_instance(&entry, event_loop)?;
        let (debug_messenger, debug_messenger_loader) =
            Self::create_debug_messenger(&entry, &instance)?;

        Ok(Self {
            entry,
            instance,
            debug_messenger,
            debug_messenger_loader,
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
            Self::check_required_validation_layers(entry)?;
        }

        let app_info = vk::ApplicationInfo::default();

        let req_ext_names = Self::get_required_extension_names(event_loop)?;
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

        match ENABLE_VALIDATION_LAYERS {
            true => {
                let info = debug_utils_messenger_create_info();
                let debug_messenger = unsafe {
                    debug_messenger_loader
                        .create_debug_utils_messenger(&info, None)?
                };
                Ok((debug_messenger, debug_messenger_loader))
            }
            false => {
                Ok((vk::DebugUtilsMessengerEXT::null(), debug_messenger_loader))
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
}


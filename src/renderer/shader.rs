use ash::vk;
use bevy::log;
use color_eyre::eyre::{Context, OptionExt, Result};
use std::{fs::File, io::Read, path::PathBuf};

use super::SHADERBUILD_DIR;

#[derive(Clone)]
pub struct GraphicsShader {
    pub vert_shader_mod: vk::ShaderModule,
    pub frag_shader_mod: vk::ShaderModule,
}

impl GraphicsShader {
    pub fn new(shadername: &str, device: &ash::Device) -> Result<Self> {
        let shaderbuild_dir = unsafe {
            SHADERBUILD_DIR
                .as_ref()
                .ok_or_eyre("Shader build directory not specified")?
        };

        let mut vert_filepath = PathBuf::from(shaderbuild_dir);
        vert_filepath.push(format!("{}-vert.spv", shadername));
        let mut frag_filepath = PathBuf::from(shaderbuild_dir);
        frag_filepath.push(format!("{}-frag.spv", shadername));

        let mut vert_spv = Vec::new();
        let mut vert_file = File::open(&vert_filepath).with_context(|| {
            format!("Failed to open file: {:#?}", vert_filepath)
        })?;
        vert_file.read_to_end(&mut vert_spv).with_context(|| {
            format!("Failed to read file: {:#?}", vert_filepath)
        })?;

        let mut frag_spv = Vec::new();
        let mut frag_file = File::open(&frag_filepath).with_context(|| {
            format!("Failed to open file: {:#?}", frag_filepath)
        })?;
        frag_file.read_to_end(&mut frag_spv).with_context(|| {
            format!("Failed to read file: {:#?}", frag_filepath)
        })?;

        let vert_shader_mod = create_shader_module(device, &vert_spv)?;
        let frag_shader_mod = create_shader_module(device, &frag_spv)?;

        Ok(Self {
            vert_shader_mod,
            frag_shader_mod,
        })
    }

    pub fn cleanup(self, device: &ash::Device) {
        log::info!("Cleaning up shader ...");
        unsafe {
            device.destroy_shader_module(self.vert_shader_mod, None);
            device.destroy_shader_module(self.frag_shader_mod, None);
        }
    }
}

pub struct ComputeShader {
    pub shader_mod: vk::ShaderModule,
}

impl ComputeShader {
    pub fn new(shadername: &str, device: &ash::Device) -> Result<Self> {
        let shaderbuild_dir = unsafe {
            SHADERBUILD_DIR
                .as_ref()
                .ok_or_eyre("Shader build directory not specified")?
        };

        let mut filepath = PathBuf::from(shaderbuild_dir);
        filepath.push(format!("{}-comp.spv", shadername));
        let mut file = File::open(&filepath)
            .with_context(|| format!("Failed to open file: {:#?}", filepath))?;
        let mut spv = Vec::new();
        file.read_to_end(&mut spv)
            .with_context(|| format!("Failed to read file: {:#?}", filepath))?;
        let shader_mod = create_shader_module(device, &spv)?;

        Ok(Self { shader_mod })
    }

    pub fn cleanup(self, device: &ash::Device) {
        unsafe {
            device.destroy_shader_module(self.shader_mod, None);
        }
    }
}

fn create_shader_module(
    device: &ash::Device,
    code: &[u8],
) -> Result<vk::ShaderModule> {
    let create_info = vk::ShaderModuleCreateInfo {
        code_size: code.len(),
        p_code: code.as_ptr() as *const u32,
        ..Default::default()
    };

    let shader_module =
        unsafe { device.create_shader_module(&create_info, None)? };

    Ok(shader_module)
}

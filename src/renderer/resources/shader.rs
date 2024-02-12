use ash::vk;
use color_eyre::eyre::{Context, OptionExt, Result};
use std::{fs::File, io::Read, path::{Path, PathBuf}};

pub static mut SHADERBUILD_DIR: Option<String> = None;

pub struct Shader {
    pub vert_shader_mod: vk::ShaderModule,
    pub frag_shader_mod: vk::ShaderModule,
}

impl Shader {
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

        let vert_shader_mod = Self::create_shader_module(device, &vert_spv)?;
        let frag_shader_mod = Self::create_shader_module(device, &frag_spv)?;

        Ok(Self {
            vert_shader_mod,
            frag_shader_mod,
        })
    }

    fn create_shader_module(
        device: &ash::Device,
        code: &Vec<u8>,
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

    pub fn destroy(self, device: &ash::Device) {
        log::info!("Cleaning up shader ...");
        unsafe {
            device.destroy_shader_module(self.vert_shader_mod, None);
            device.destroy_shader_module(self.frag_shader_mod, None);
        }
    }
}

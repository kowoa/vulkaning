use anyhow::{anyhow, Context};
use ash::vk;
use std::fs::File;
use std::io::Read;

const SHADERBUILD_DIR: &'static str = "./shaderbuild";

pub struct Shader {
    vert_spv: Vec<u8>,
    frag_spv: Vec<u8>,
}

impl Shader {
    pub fn new(shadername: &str) -> anyhow::Result<Self> {
        let vert_filepath =
            format!("{}/{}-vert.spv", SHADERBUILD_DIR, shadername);
        let frag_filepath =
            format!("{}/{}-frag.spv", SHADERBUILD_DIR, shadername);

        let mut vert_spv = Vec::new();
        let mut vert_file = File::open(&vert_filepath).with_context(|| {
            format!("Failed to open file: {}", vert_filepath)
        })?;
        vert_file.read_to_end(&mut vert_spv).with_context(|| {
            format!("Failed to read file: {}", vert_filepath)
        })?;

        let mut frag_spv = Vec::new();
        let mut frag_file = File::open(&frag_filepath).with_context(|| {
            format!("Failed to open file: {}", frag_filepath)
        })?;
        frag_file.read_to_end(&mut frag_spv).with_context(|| {
            format!("Failed to read file: {}", frag_filepath)
        })?;

        Ok(Self { vert_spv, frag_spv })
    }

    pub fn create_shader_module_vert(
        &self,
        device: &ash::Device,
    ) -> anyhow::Result<vk::ShaderModule> {
        Self::create_shader_module(device, self.vert_spv)
    }

    pub fn create_shader_module_frag(
        &self,
        device: &ash::Device,
    ) -> anyhow::Result<vk::ShaderModule> {
        Self::create_shader_module(device, self.frag_spv)
    }

    fn create_shader_module(
        device: &ash::Device,
        code: Vec<u8>,
    ) -> anyhow::Result<vk::ShaderModule> {
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as *const u32,
            ..Default::default()
        };

        let shader_module = unsafe {
            device.create_shader_module(&create_info, None)?
        };

        Ok(shader_module)
    }
}

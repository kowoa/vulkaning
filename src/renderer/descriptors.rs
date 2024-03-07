use std::collections::HashMap;

use ash::vk;
use color_eyre::eyre::{OptionExt, Result};

pub struct DescriptorSetLayoutBuilder {
    bindings: Vec<vk::DescriptorSetLayoutBinding>,
}

impl DescriptorSetLayoutBuilder {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn add_binding(
        mut self,
        binding: u32,
        desc_type: vk::DescriptorType,
        stage_flags: vk::ShaderStageFlags,
    ) -> Self {
        self.bindings.push(
            vk::DescriptorSetLayoutBinding::builder()
                .binding(binding)
                .descriptor_type(desc_type)
                .descriptor_count(1)
                .stage_flags(stage_flags)
                .build(),
        );
        self
    }

    pub fn clear(mut self) -> Self {
        self.bindings.clear();
        self
    }

    pub fn build(
        self,
        device: &ash::Device,
    ) -> Result<vk::DescriptorSetLayout> {
        let info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&self.bindings)
            .build();
        Ok(unsafe { device.create_descriptor_set_layout(&info, None)? })
    }
}

pub struct PoolSizeRatio {
    pub desc_type: vk::DescriptorType,
    pub ratio: f32,
}

#[derive(Debug)]
pub struct DescriptorAllocator {
    pub pool: vk::DescriptorPool,
    layouts: HashMap<String, vk::DescriptorSetLayout>,
}

impl DescriptorAllocator {
    pub fn new(
        device: &ash::Device,
        max_sets: u32,
        pool_ratios: &[PoolSizeRatio],
    ) -> Result<Self> {
        let pool_sizes = pool_ratios
            .iter()
            .map(|ratio| vk::DescriptorPoolSize {
                ty: ratio.desc_type,
                descriptor_count: (ratio.ratio * max_sets as f32) as u32,
            })
            .collect::<Vec<vk::DescriptorPoolSize>>();

        let pool_info = vk::DescriptorPoolCreateInfo {
            max_sets,
            pool_size_count: pool_sizes.len() as u32,
            p_pool_sizes: pool_sizes.as_ptr(),
            ..Default::default()
        };
        let pool = unsafe { device.create_descriptor_pool(&pool_info, None)? };

        Ok(Self {
            pool,
            layouts: HashMap::new(),
        })
    }

    pub fn clear_descriptors(&mut self, device: &ash::Device) -> Result<()> {
        unsafe {
            device.reset_descriptor_pool(
                self.pool,
                vk::DescriptorPoolResetFlags::empty(),
            )?;
        }

        Ok(())
    }

    pub fn add_layout(&mut self, name: &str, layout: vk::DescriptorSetLayout) {
        self.layouts.insert(name.into(), layout);
    }

    pub fn get_layout(&self, name: &str) -> Result<&vk::DescriptorSetLayout> {
        self.layouts
            .get(name)
            .ok_or_eyre(format!("Descriptor Set Layout not found: {}", name))
    }

    pub fn allocate(
        &self,
        device: &ash::Device,
        layout_name: &str,
    ) -> Result<vk::DescriptorSet> {
        let layout = self.get_layout(layout_name)?;
        let layouts = [*layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.pool)
            .set_layouts(&layouts)
            .build();
        Ok(unsafe { device.allocate_descriptor_sets(&alloc_info)?[0] })
    }

    pub fn cleanup(self, device: &ash::Device) {
        unsafe {
            device.destroy_descriptor_pool(self.pool, None);
            for (_, layout) in self.layouts {
                device.destroy_descriptor_set_layout(layout, None);
            }
        }
    }
}

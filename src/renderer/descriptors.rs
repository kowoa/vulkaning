use ash::vk;
use color_eyre::eyre::Result;

pub struct DescriptorLayoutBuilder {
    bindings: Vec<vk::DescriptorSetLayoutBinding>,
}

impl DescriptorLayoutBuilder {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn add_binding(
        mut self,
        binding: u32,
        desc_type: vk::DescriptorType,
    ) -> Self {
        self.bindings.push(vk::DescriptorSetLayoutBinding {
            binding,
            descriptor_type: desc_type,
            descriptor_count: 1,
            ..Default::default()
        });
        self
    }

    pub fn clear(mut self) -> Self {
        self.bindings.clear();
        self
    }

    pub fn build(
        mut self,
        device: &ash::Device,
        shader_stages: vk::ShaderStageFlags,
    ) -> Result<vk::DescriptorSetLayout> {
        for binding in &mut self.bindings {
            binding.stage_flags |= shader_stages;
        }

        let info = vk::DescriptorSetLayoutCreateInfo {
            p_bindings: self.bindings.as_ptr(),
            binding_count: self.bindings.len() as u32,
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            ..Default::default()
        };

        Ok(unsafe { device.create_descriptor_set_layout(&info, None)? })
    }
}

pub struct PoolSizeRatio {
    desc_type: vk::DescriptorType,
    ratio: f32,
}

pub struct DescriptorAllocator {
    pub pool: vk::DescriptorPool,
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

        Ok(Self { pool })
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

    pub fn allocate(
        &self,
        device: &ash::Device,
        layout: vk::DescriptorSetLayout,
    ) -> Result<vk::DescriptorSet> {
        let alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: self.pool,
            descriptor_set_count: 1,
            p_set_layouts: &layout,
            ..Default::default()
        };

        let desc_set =
            unsafe { device.allocate_descriptor_sets(&alloc_info)?[0] };

        Ok(desc_set)
    }

    pub fn cleanup(self, device: &ash::Device) {
        unsafe {
            device.destroy_descriptor_pool(self.pool, None);
        }
    }
}

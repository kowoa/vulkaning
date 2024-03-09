use std::collections::HashMap;

use ash::vk;
use color_eyre::eyre::{eyre, OptionExt, Result};

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

#[derive(Debug, Clone)]
pub struct PoolSizeRatio {
    pub desc_type: vk::DescriptorType,
    pub ratio: f32,
}

#[derive(Debug)]
pub struct DescriptorAllocator {
    layouts: HashMap<String, vk::DescriptorSetLayout>,

    pool_ratios: Vec<PoolSizeRatio>, // Needed to reallocate pools
    full_pools: Vec<vk::DescriptorPool>, // Pools that cannot allocate more sets
    ready_pools: Vec<vk::DescriptorPool>, // Pools that can allocate more sets
    sets_per_pool: u32,
}

impl DescriptorAllocator {
    pub fn new(
        device: &ash::Device,
        max_sets: u32,
        pool_ratios: &[PoolSizeRatio],
    ) -> Result<Self> {
        // Allocate the first descriptor pool and add it to ready_pools
        let new_pool = Self::create_pool(device, max_sets, pool_ratios)?;
        let ready_pools = vec![new_pool];
        // Incrase number of sets per pool by 50% for the next pool allocation
        let sets_per_pool = (max_sets as f32 * 1.5) as u32;

        Ok(Self {
            layouts: HashMap::new(),
            pool_ratios: pool_ratios.to_vec(),
            full_pools: Vec::new(),
            ready_pools,
            sets_per_pool,
        })
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
        &mut self,
        device: &ash::Device,
        layout_name: &str,
    ) -> Result<vk::DescriptorSet> {
        let set_layouts = [*self.get_layout(layout_name)?];
        let mut pool_to_use = self.get_pool(device)?;

        let mut alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: pool_to_use,
            descriptor_set_count: 1,
            p_set_layouts: set_layouts.as_ptr(),
            ..Default::default()
        };

        let desc_set = match unsafe {
            device.allocate_descriptor_sets(&alloc_info)
        } {
            Ok(desc_set) => Ok(desc_set[0]),
            Err(err) => {
                // If the pool is full, push the pool to full_pools and get a new pool
                if err == vk::Result::ERROR_OUT_OF_POOL_MEMORY
                    || err == vk::Result::ERROR_FRAGMENTED_POOL
                {
                    self.full_pools.push(pool_to_use);
                    pool_to_use = self.get_pool(device)?;
                    alloc_info.descriptor_pool = pool_to_use;
                    // If getting a new pool fails, don't try again because stuff is broken
                    Ok(unsafe {
                        device.allocate_descriptor_sets(&alloc_info)?[0]
                    })
                } else {
                    Err(eyre!("Failed to allocate descriptor set: {:?}", err))
                }
            }
        }?;
        self.ready_pools.push(pool_to_use);

        Ok(desc_set)
    }

    pub fn clear_pools(&mut self, device: &ash::Device) -> Result<()> {
        for pool in self.ready_pools.iter() {
            unsafe {
                device.reset_descriptor_pool(
                    *pool,
                    vk::DescriptorPoolResetFlags::empty(),
                )?;
            }
        }

        for pool in self.full_pools.drain(..) {
            unsafe {
                device.reset_descriptor_pool(
                    pool,
                    vk::DescriptorPoolResetFlags::empty(),
                )?;
                self.ready_pools.push(pool);
            }
        }

        Ok(())
    }

    pub fn destroy_pools(&mut self, device: &ash::Device) {
        for pool in self.ready_pools.drain(..) {
            unsafe {
                device.destroy_descriptor_pool(pool, None);
            }
        }

        for pool in self.full_pools.drain(..) {
            unsafe {
                device.destroy_descriptor_pool(pool, None);
            }
        }
    }

    pub fn cleanup(mut self, device: &ash::Device) {
        for layout in self.layouts.values() {
            unsafe {
                device.destroy_descriptor_set_layout(*layout, None);
            }
        }
        self.destroy_pools(device);
    }

    fn get_pool(&mut self, device: &ash::Device) -> Result<vk::DescriptorPool> {
        if let Some(ready_pool) = self.ready_pools.pop() {
            Ok(ready_pool)
        } else {
            // Ran out of pools
            let new_pool = Self::create_pool(
                device,
                self.sets_per_pool,
                &self.pool_ratios,
            );

            // Increase number of sets per pool
            let sets_per_pool = (self.sets_per_pool as f32 * 1.5) as u32;
            self.sets_per_pool = sets_per_pool.min(4092); // Limit max sets per pool
            new_pool
        }
    }

    fn create_pool(
        device: &ash::Device,
        set_count: u32,
        ratios: &[PoolSizeRatio],
    ) -> Result<vk::DescriptorPool> {
        let pool_sizes = ratios
            .iter()
            .map(|ratio| vk::DescriptorPoolSize {
                ty: ratio.desc_type,
                descriptor_count: (ratio.ratio * set_count as f32) as u32,
            })
            .collect::<Vec<vk::DescriptorPoolSize>>();

        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .max_sets(set_count)
            .pool_sizes(&pool_sizes)
            .build();

        Ok(unsafe { device.create_descriptor_pool(&pool_info, None)? })
    }
}

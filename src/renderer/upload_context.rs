use ash::vk;
use color_eyre::eyre::Result;

use super::vkinit;

pub struct UploadContext {
    upload_fence: vk::Fence,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    queue: vk::Queue,
}

impl UploadContext {
    pub fn new(
        device: &ash::Device,
        queue_family_index: u32,
        queue: vk::Queue,
    ) -> Result<Self> {
        let upload_fence_info = vk::FenceCreateInfo::default();
        let upload_fence =
            unsafe { device.create_fence(&upload_fence_info, None)? };

        let command_pool_info = vk::CommandPoolCreateInfo {
            queue_family_index,
            // Allow the pool to reset individual command buffers
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            ..Default::default()
        };
        let command_pool =
            unsafe { device.create_command_pool(&command_pool_info, None)? };

        let command_buffer_info = vk::CommandBufferAllocateInfo {
            command_pool,
            command_buffer_count: 1,
            level: vk::CommandBufferLevel::PRIMARY,
            ..Default::default()
        };
        let command_buffer = unsafe {
            device.allocate_command_buffers(&command_buffer_info)?[0]
        };

        Ok(Self {
            upload_fence,
            command_pool,
            command_buffer,
            queue,
        })
    }

    pub fn cleanup(self, device: &ash::Device) {
        unsafe {
            device.destroy_command_pool(self.command_pool, None);
            device.destroy_fence(self.upload_fence, None);
        }
    }

    // Instantly execute some commands to the GPU without dealing with the render loop and other synchronization
    // This is great for compute calculations and can be used from a background thread separated from the render loop
    pub fn immediate_submit<F>(
        &self,
        func: F,
        device: &ash::Device,
    ) -> Result<()>
    where
        F: Fn(&vk::CommandBuffer, &ash::Device),
    {
        let cmd = self.command_buffer;

        // This command buffer will be used exactly once before resetting
        let cmd_begin_info = vkinit::command_buffer_begin_info(
            vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
        );
        // Begin the command buffer recording
        unsafe {
            device.begin_command_buffer(cmd, &cmd_begin_info)?;
        }

        func(&cmd, device);

        // End the command buffer recording
        unsafe {
            device.end_command_buffer(cmd)?;
        }

        // Submit command buffer to the queue and execute it
        let submit = vkinit::submit_info(&cmd);
        unsafe {
            device.queue_submit(self.queue, &[submit], self.upload_fence)?;
        }

        unsafe {
            // upload_fence will now block until the graphics commands finish execution
            device.wait_for_fences(&[self.upload_fence], true, 9999999999)?;
            device.reset_fences(&[self.upload_fence])?;
            // Reset command buffers inside command pool
            device.reset_command_pool(
                self.command_pool,
                vk::CommandPoolResetFlags::empty(),
            )?;
        }

        Ok(())
    }
}

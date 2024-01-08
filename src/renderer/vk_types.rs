use ash::vk;
use gpu_allocator::vulkan::Allocation;

pub struct AllocatedBuffer {
    buffer: vk::Buffer,
    allocation: Allocation,
}
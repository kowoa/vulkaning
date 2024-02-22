use ash::vk;
use color_eyre::eyre::Result;
use std::ffi::{c_char, CStr};

pub fn c_char_to_string(c_char_array: &[c_char]) -> Result<String> {
    let cstr = unsafe { CStr::from_ptr(c_char_array.as_ptr()) };
    Ok(cstr.to_str()?.to_string())
}

pub fn pad_uniform_buffer_size(
    original_size: u64,
    min_uniform_buffer_offset_alignment: u64,
) -> u64 {
    // Calculate required alignment based on minimum device offset alignment
    if min_uniform_buffer_offset_alignment > 0 {
        (original_size + min_uniform_buffer_offset_alignment - 1)
            & !(min_uniform_buffer_offset_alignment - 1)
    } else {
        original_size
    }
}

pub fn copy_image_to_image(
    src: vk::Image,
    dst: vk::Image,
    src_size: vk::Extent2D,
    dst_size: vk::Extent2D,
    device: &ash::Device,
    cmd: vk::CommandBuffer,
) {
    let blit_region = vk::ImageBlit2 {
        src_offsets: [
            vk::Offset3D { x: 0, y: 0, z: 0 },
            vk::Offset3D {
                x: src_size.width as i32,
                y: src_size.height as i32,
                z: 1,
            },
        ],
        dst_offsets: [
            vk::Offset3D { x: 0, y: 0, z: 0 },
            vk::Offset3D {
                x: dst_size.width as i32,
                y: dst_size.height as i32,
                z: 1,
            },
        ],
        src_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_array_layer: 0,
            layer_count: 1,
            mip_level: 0,
        },
        dst_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_array_layer: 0,
            layer_count: 1,
            mip_level: 0,
        },
        ..Default::default()
    };

    let blit_info = vk::BlitImageInfo2 {
        dst_image: dst,
        dst_image_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        src_image: src,
        src_image_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        filter: vk::Filter::LINEAR,
        region_count: 1,
        p_regions: &blit_region,
        ..Default::default()
    };

    unsafe {
        device.cmd_blit_image2(cmd, &blit_info);
    }
}

#[cfg(test)]
mod tests {
    use crate::renderer::vkutils::pad_uniform_buffer_size;

    #[test]
    fn test_pad_uniform_buffer_32_size_0_alignment() {
        assert_eq!(pad_uniform_buffer_size(32, 0), 32);
    }

    #[test]
    fn test_pad_uniform_buffer_32_size_32_alignment() {
        assert_eq!(pad_uniform_buffer_size(32, 32), 32);
    }

    #[test]
    fn test_pad_uniform_buffer_32_size_64_alignment() {
        assert_eq!(pad_uniform_buffer_size(32, 64), 64);
    }

    #[test]
    fn test_pad_uniform_buffer_32_size_54_alignment() {
        assert_eq!(pad_uniform_buffer_size(32, 54), 64);
    }

    #[test]
    fn test_pad_uniform_buffer_22_size_64_alignment() {
        assert_eq!(pad_uniform_buffer_size(22, 64), 64);
    }

    #[test]
    fn test_pad_uniform_buffer_22_size_54_alignment() {
        assert_eq!(pad_uniform_buffer_size(22, 54), 74);
    }
}

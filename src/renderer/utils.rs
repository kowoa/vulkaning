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

#[cfg(test)]
mod tests {
    use crate::renderer::utils::pad_uniform_buffer_size;

    #[test]
    fn test_pad_uniform_buffer_size_0_alignment() {
        assert_eq!(pad_uniform_buffer_size(32, 0), 32);
    }

    #[test]
    fn test_pad_uniform_buffer_size_32_alignment() {
        assert_eq!(pad_uniform_buffer_size(32, 32), 32);
    }

    #[test]
    fn test_pad_uniform_buffer_size_64_alignment() {
        assert_eq!(pad_uniform_buffer_size(32, 64), 64);
    }
}

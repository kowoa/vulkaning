use std::ffi::{c_char, CStr};

pub fn c_char_to_string(c_char_array: &[c_char]) -> anyhow::Result<String> {
    let cstr = unsafe { CStr::from_ptr(c_char_array.as_ptr()) };
    Ok(cstr.to_str()?.to_string())
}


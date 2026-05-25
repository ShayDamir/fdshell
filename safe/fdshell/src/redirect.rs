#![forbid(unsafe_code)]

use std::ffi::CString;

pub struct Redirect {
    pub target_fd: i32,
    pub src_var: CString,
}

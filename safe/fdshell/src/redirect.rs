#![forbid(unsafe_code)]

use std::ffi::CString;

pub struct Redirect {
    pub target_fd: i32,
    pub src_var: CString,
}

impl PartialEq for Redirect {
    fn eq(&self, other: &Self) -> bool {
        self.target_fd == other.target_fd && self.src_var == other.src_var
    }
}

impl core::fmt::Debug for Redirect {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Redirect")
            .field("target_fd", &self.target_fd)
            .field("src_var", &self.src_var)
            .finish()
    }
}

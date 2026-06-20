#![forbid(unsafe_code)]

//! File descriptor pass-through errors (child/fdpass.rs).

use displaydoc::Display;

/// [FdPassError] FD pass-through errors
#[derive(Display, Debug)]
pub(crate) enum FdPassError {
    /// missing argument
    MissingArg,
    /// invalid fd variable name prefix
    InvalidName,
    /// fd not found in state
    NotFound,
    /// fd send failed
    SendFailed,
}

impl core::error::Error for FdPassError {}

impl From<i32> for FdPassError {
    fn from(_: i32) -> Self {
        FdPassError::SendFailed
    }
}

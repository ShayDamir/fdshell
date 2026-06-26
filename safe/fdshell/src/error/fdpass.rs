//! File descriptor pass-through errors (child/fdpass.rs).

/// [FdPassError] FD pass-through errors
#[derive(displaydoc::Display, Debug)]
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

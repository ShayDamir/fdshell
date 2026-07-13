//! Shell initialization errors (init.rs).

/// [ShellInitError] Shell initialization errors
#[derive(displaydoc::Display, Debug)]
pub enum ShellInitError {
    /// capture fd is not valid (not open or has CLOEXEC)
    NestedFd,
}

impl core::error::Error for ShellInitError {}

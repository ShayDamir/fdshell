#![forbid(unsafe_code)]

//! Shell initialization errors (init.rs).

use displaydoc::Display;

/// [ShellInitError] Shell initialization errors
#[derive(Display, Debug)]
pub(crate) enum ShellInitError {
    /// capture fd is not valid (not open or has CLOEXEC)
    NestedFd,
    /// shell socketpair setup failed
    ShellSocket,
}

impl core::error::Error for ShellInitError {}

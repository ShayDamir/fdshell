#![forbid(unsafe_code)]

//! Launch / child execution errors (launch.rs, child/*.rs).

use displaydoc::Display;

/// [LaunchError] Launch / child execution errors
#[derive(Display, Debug)]
pub(crate) enum LaunchError {
    /// fork syscall failed
    #[displaydoc("fork failed")]
    Fork,
    /// child exec syscall failed
    #[displaydoc("exec failed")]
    Exec,
    /// builtin dispatch in child failed
    #[displaydoc("builtin dispatch failed")]
    BuiltinDispatch,
}

impl core::error::Error for LaunchError {}

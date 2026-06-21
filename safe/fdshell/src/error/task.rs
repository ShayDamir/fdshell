#![forbid(unsafe_code)]

//! Task management errors (task.rs).

use displaydoc::Display;

/// [TaskError] Task management errors
#[derive(Display, Debug)]
pub(crate) enum TaskError {
    /// task key argument must start with '&'
    BadArg,
    /// task not found
    NotFound,
    /// wait syscall failed
    Wait,
}

impl core::error::Error for TaskError {}

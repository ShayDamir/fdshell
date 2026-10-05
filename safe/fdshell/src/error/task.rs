//! Task management errors (task.rs).

/// [TaskError] Task management errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum TaskError {
    /// task key argument must start with '&'
    BadArg,
    /// task not found
    NotFound,
    /// wait: '{arg}' is not a pid of a background task
    BadPid { arg: sys::ShortCStr },
    /// wait syscall failed
    Wait,
    /// failed to collect captured output
    Capture,
}

impl core::error::Error for TaskError {}

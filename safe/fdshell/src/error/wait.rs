//! `wait` event-case round errors.

use sys::ShortCStr;

/// [WaitError] `wait` round errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum WaitError {
    /// poll failed
    Poll,
    /// `wait` has nothing to poll (no ready-capable arm)
    EmptyPoll,
    /// {name} is not set
    NoFd { name: ShortCStr },
    /// {name} is not an fd array
    NotAnArray { name: ShortCStr },
    /// {name} is not a background task
    TaskNotFound { name: ShortCStr },
    /// `wait` arm fork failed
    ArmFork,
    /// draining a `wait` arm's captures failed
    ArmCapture,
    /// reaping a background task failed
    Reap,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for WaitError {}

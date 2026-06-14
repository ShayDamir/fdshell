#![forbid(unsafe_code)]

use displaydoc::Display;

/// Top-level error for fdshell
#[derive(Debug, Display)]
pub(crate) enum AppError {
    /// initialization failed
    Init,
    /// command execution failed
    Exec,
    /// CWD directory open failed
    Cwd,
    /// input read error
    Read,
    /// state borrow failed
    Borrow,
    /// usage: fdshell [-c command]
    Usage,
}

impl core::error::Error for AppError {}

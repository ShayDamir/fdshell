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
    /// usage: fdshell [-c command] [name args...] or fdshell script.sh [args...]
    Usage,
    /// failed to read script file
    ScriptRead,
    /// invalid UTF-8 in {field}
    InvalidUtf8 { field: &'static str },
}

impl core::error::Error for AppError {}

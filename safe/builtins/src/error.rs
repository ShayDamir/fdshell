#![forbid(unsafe_code)]

//! Typed errors for builtin commands.
//!
//! Replaces raw `i32` errno returns so callers can match on the specific
//! problem instead of decoding integer constants.

use core::fmt;

/// Errors that can occur in builtin command dispatch and execution.
///
/// Each variant describes a single, distinct cause.
#[derive(Debug, PartialEq)]
pub enum BuiltinError {
    /// User requested help text with `--help` / `-h`.
    Help,
    /// Invalid argument, flag, or value.
    InvalidArgument,
    /// Underlying syscall failed.
    Syscall(sys::SyscallError),
    /// No builtin matches the given command name.
    Unknown,
}

impl fmt::Display for BuiltinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help => write!(f, "help requested"),
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::Syscall(e) => write!(f, "syscall failed: {}", e),
            Self::Unknown => write!(f, "unknown builtin"),
        }
    }
}

impl core::error::Error for BuiltinError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Syscall(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sys::SyscallError> for BuiltinError {
    fn from(e: sys::SyscallError) -> Self {
        BuiltinError::Syscall(e)
    }
}

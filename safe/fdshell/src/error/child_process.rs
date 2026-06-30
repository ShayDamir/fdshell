//! Child process execution errors.
//!
//! Covers failures in forked child processes and the replacer (`exec`) command.

use sys::ShortCStr;

/// [ChildProcessError] Child process execution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ChildProcessError {
    /// redirect in child failed
    RedirectFailed,
    /// argument substitution in child failed
    SubstituteFailed,
    /// state borrow in child failed
    BorrowFailed,
    /// "{0}" is not a shell builtin
    NotABuiltin(ShortCStr),
    /// failed to resolve command path: "{0}"
    ResolveFailed(ShortCStr),
    /// "{0}" not found
    NotFound(ShortCStr),
    /// execveat failed
    ExecFailed,
    /// builtin execution failed
    BuiltinExecutionFailed,
    /// missing argument
    MissingArg,
    /// fd export for execveat failed
    ExportFailed,
    /// impossible error state (should never occur)
    Never,
}

impl ChildProcessError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound(_) => 127,
            Self::NotABuiltin(_)
            | Self::RedirectFailed
            | Self::SubstituteFailed
            | Self::BorrowFailed
            | Self::ResolveFailed(_)
            | Self::ExecFailed
            | Self::BuiltinExecutionFailed
            | Self::MissingArg
            | Self::ExportFailed
            | Self::Never => 1,
        }
    }
}

impl core::error::Error for ChildProcessError {}

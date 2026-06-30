//! Exec (execveat, resolve_path) errors.

use sys::ShortCStr;

/// [ExecError] Exec / path resolution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ExecError {
    /// "{0}" not found
    NotFound(ShortCStr),
    /// "{0}" is not a shell builtin
    NotABuiltin(ShortCStr),
    /// missing argument
    MissingArg,
    /// fd export for execveat failed
    ExportFailed,
    /// execveat failed
    ExecFailed,
    /// builtin execution failed
    BuiltinExecutionFailed,
    /// impossible error state (should never occur)
    Never,
}

impl ExecError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound(_) => 127,
            Self::MissingArg
            | Self::ExportFailed
            | Self::ExecFailed
            | Self::NotABuiltin(_)
            | Self::BuiltinExecutionFailed
            | Self::Never => 1,
        }
    }
}

impl core::error::Error for ExecError {}

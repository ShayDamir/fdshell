//! Exec (execveat, resolve_path) errors.

/// [ExecError] Exec / path resolution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ExecError {
    /// command not found
    NotFound,
    /// not a shell builtin
    NotABuiltin,
    /// missing argument
    MissingArg,
    /// fd export for execveat failed
    ExportFailed,
    /// execveat failed
    ExecFailed,
}

impl ExecError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound => 127,
            Self::MissingArg | Self::ExportFailed | Self::ExecFailed | Self::NotABuiltin => 1,
        }
    }
}

impl core::error::Error for ExecError {}

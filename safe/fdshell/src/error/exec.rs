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
    /// I/O error in builtin
    Io,
}

impl ExecError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound => 127,
            Self::MissingArg
            | Self::ExportFailed
            | Self::ExecFailed
            | Self::NotABuiltin
            | Self::Io => 1,
        }
    }
}

impl core::error::Error for ExecError {}

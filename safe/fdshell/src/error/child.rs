//! Child process execution errors (child/run.rs, pipeline/child.rs).

/// [ChildError] Child process execution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ChildError {
    /// redirect in child failed
    RedirectFailed,
    /// argument substitution in child failed
    SubstituteFailed,
    /// state borrow in child failed
    BorrowFailed,
    /// not a shell builtin
    NotABuiltin,
    /// failed to resolve command path
    NotFound,
    /// execveat in child failed
    ExecFailed,
    /// I/O error in builtin
    Io,
}

impl ChildError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound => 127,
            Self::NotABuiltin
            | Self::RedirectFailed
            | Self::SubstituteFailed
            | Self::BorrowFailed
            | Self::ExecFailed
            | Self::Io => 1,
        }
    }
}

impl core::error::Error for ChildError {}

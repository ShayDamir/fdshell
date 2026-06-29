//! Child process execution errors (child/run.rs, pipeline/child.rs).

use sys::ShortCStr;

/// [ChildError] Child process execution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ChildError {
    /// redirect in child failed
    RedirectFailed,
    /// argument substitution in child failed
    SubstituteFailed,
    /// state borrow in child failed
    BorrowFailed,
    /// "{0}" is not a shell builtin
    NotABuiltin(ShortCStr),
    /// failed to resolve command path: "{0}"
    NotFound(ShortCStr),
    /// execveat in child failed
    ExecFailed,
    /// I/O error in builtin
    Io,
}

impl ChildError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound(_) => 127,
            Self::NotABuiltin(_)
            | Self::RedirectFailed
            | Self::SubstituteFailed
            | Self::BorrowFailed
            | Self::ExecFailed
            | Self::Io => 1,
        }
    }
}

impl core::error::Error for ChildError {}

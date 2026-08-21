//! Redirection file opening errors (redirect/open.rs, redirect/resolve.rs).

/// [OpenRedirectError] Redirection file opening errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum OpenRedirectError {
    /// failed to open redirection path
    Open,
    /// fd variable '{var}' is not set
    VarNotFound { var: sys::ShortCStr },
    /// file descriptor number is out of range
    FdNumberOutOfRange,
}

impl core::error::Error for OpenRedirectError {}

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
    /// here-string: failed to expand the word
    HereStringExpand,
    /// here-string: failed to create the stdin file
    HereStringCreate,
    /// fd {n} is not open; `>&{n}` has nothing to duplicate
    FdNotOpen { n: i32 },
    /// failed to duplicate fd {n}; the descriptor table may be full
    DupFdFailed { n: i32 },
    /// failed to close redirection target fd {n}
    CloseFd { n: i32 },
    /// internal invariant violated
    Never,
}

impl core::error::Error for OpenRedirectError {}

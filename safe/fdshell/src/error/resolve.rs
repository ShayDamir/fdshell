#![forbid(unsafe_code)]

//! File descriptor resolution errors (redirect/resolve.rs, substitute/).

use displaydoc::Display;

/// [ResolveError] FD resolution errors
#[derive(Display, Debug)]
pub(crate) enum ResolveError {
    /// variable or file reference not found
    #[displaydoc("reference not found")]
    RefNotFound,
    /// duplicate redirect target in a single command
    #[displaydoc("duplicate redirect target")]
    RedirectDuplicate,
    /// file descriptor resolution failed
    #[displaydoc("resolution failed")]
    Resolve,
}

impl core::error::Error for ResolveError {}

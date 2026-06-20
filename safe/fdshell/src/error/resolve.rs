#![forbid(unsafe_code)]

//! File descriptor resolution errors (redirect/resolve.rs, substitute/).

use displaydoc::Display;

/// [ResolveError] FD resolution errors
#[derive(Display, Debug)]
pub(crate) enum ResolveError {
    /// variable or file reference not found
    RefNotFound,
    /// token too long for variable storage
    TokenTooLong,
    /// NUL byte in variable name
    NulByte,
    /// unclosed subexpression parenthesis
    UnclosedParen,
    /// resolution failed
    Resolve,
}

impl core::error::Error for ResolveError {}

/// Convert `i32` (errno) to `ResolveError::Resolve`.
impl From<i32> for ResolveError {
    fn from(_: i32) -> Self {
        ResolveError::Resolve
    }
}

impl From<crate::error::cmd_subst::CmdSubstError> for ResolveError {
    fn from(_: crate::error::cmd_subst::CmdSubstError) -> Self {
        ResolveError::Resolve
    }
}

//! File descriptor resolution errors (redirect/resolve.rs, substitute/).

/// [ResolveError] FD resolution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ResolveError {
    /// variable or file reference not found
    RefNotFound,
    /// NUL byte in variable name
    NulByte,
    /// unclosed subexpression parenthesis
    UnclosedParen,
    /// malformed variable/reference syntax
    MalformedRef,
    /// resolution failed
    Resolve,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for ResolveError {}

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
    /// resolution failed
    Resolve,
}

impl core::error::Error for ResolveError {}

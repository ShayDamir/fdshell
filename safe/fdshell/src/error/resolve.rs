//! File descriptor resolution errors (redirect/resolve.rs, substitute/).

use sys::ShortCStr;

/// [ResolveError] FD resolution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ResolveError {
    /// variable or file reference not found
    RefNotFound,
    /// {var}: {word}
    ParamNullOrNotSet { var: ShortCStr, word: ShortCStr },
    /// NUL byte in variable name
    NulByte,
    /// unclosed subexpression parenthesis
    UnclosedParen,
    /// index or value too large for type
    TooLarge,
    /// resolution failed
    Resolve,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for ResolveError {}

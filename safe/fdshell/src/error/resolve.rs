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
    /// arithmetic expression has a syntax error
    ArithSyntax,
    /// division or modulo by zero in arithmetic expression
    ArithDivZero,
    /// variable {var} is not an integer
    ArithNotInteger { var: ShortCStr },
    /// variable {var} refers to itself (circular reference)
    ArithCircular { var: ShortCStr },
    /// resolution failed
    Resolve,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for ResolveError {}

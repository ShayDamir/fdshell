//! File descriptor resolution errors (redirect/resolve.rs, substitute/).

use sys::ShortCStr;

/// [ResolveError] FD resolution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ResolveError {
    /// variable or file reference not found
    RefNotFound,
    /// {var}: {word}
    ParamNullOrNotSet { var: ShortCStr, word: ShortCStr },
    /// {var}: unbound variable
    UnboundVariable { var: ShortCStr },
    /// {var}: invalid indirect expansion
    InvalidIndirect { var: ShortCStr },
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
    /// command substitution inside an arithmetic expression failed
    ArithSubst,
    /// arithmetic expression nested too deeply; reduce the nesting of $((...)) expansions
    ArithTooDeep,
    /// variable {var} is not an integer
    ArithNotInteger { var: ShortCStr },
    /// variable {var} refers to itself (circular reference)
    ArithCircular { var: ShortCStr },
    /// no match: {word}
    GlobNoMatch { word: ShortCStr },
    /// resolution failed
    Resolve,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for ResolveError {}

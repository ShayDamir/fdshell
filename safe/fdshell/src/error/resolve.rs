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
    /// {var}: cannot assign in this way
    CannotAssign { var: ShortCStr },
    /// NUL byte in variable name
    NulByte,
    /// unclosed subexpression parenthesis
    UnclosedParen,
    /// unclosed backtick command substitution
    UnclosedBacktick,
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
    // Truly never: the `write!`/`push_byte` arms can only fail on a NUL, and every
    // written value is a number (`$$`, `$#`, `$?`, an fd number, a length) or a span
    // already cleared at the ingestion point — `substitute/mask.rs` is where a NUL
    // answers `NulByte`; `arith/binop.rs`'s arm is dispatch completeness and
    // `param_op/pattern.rs`/`glob.rs` are bounds/bookkeeping. Pinned by
    // `arith/tests.rs::arith_binop_rejects_non_binary_ops`.
    // LESSONS: "A NUL byte is reachable only at the entry points that ingest outside bytes"
    // Chore #565: excluded from later respawns.
    Never,
}

impl core::error::Error for ResolveError {}

#![forbid(unsafe_code)]

//! Parser errors (parse/*.rs).
//!
//! `ParseError` covers parse-time errors with position info.
//! Execution-phase errors that lack position use the `Reason` variant.

use displaydoc::Display;

/// [ParseError] Parser errors
// Debug needed for impl Error (trait bound), not for display.
#[derive(Display, Debug)]
pub(crate) enum ParseError {
    /// unmatched quote at byte position {pos}
    UnbalancedQuote { pos: usize },
    /// unexpected end of input at byte position {pos}
    UnexpectedEof { pos: usize },
    /// NUL byte at byte position {pos}
    InvalidChar { ch: u8, pos: usize },
    /// {reason}
    Reason { pos: usize, reason: &'static str },
}

impl core::error::Error for ParseError {}

/// Convert `i32` (errno) to `ParseError`.
///
/// `ShortCStr::push` returns `EINVAL` for NUL bytes. Since there's only
/// one possible errno, we map it to `InvalidChar` with position 0.
impl From<i32> for ParseError {
    fn from(_: i32) -> Self {
        ParseError::InvalidChar { ch: 0, pos: 0 }
    }
}

/// Legacy struct for execution-phase errors without specific error kind.
///
/// This is retained for the execution chain (run.rs, cond.rs, etc.)
/// where errors originate from syscall boundaries and lack parse context.
/// It will be removed when the execution chain is migrated to `ParseError`.
// Debug needed for impl Error (trait bound), not for display.
#[derive(Debug)]
pub(crate) struct ParseErrorInfo {
    /// Byte offset of the error in the input.
    pub(crate) source_start: usize,
    /// Optional human-readable error message.
    pub(crate) message: Option<&'static str>,
}

impl ParseErrorInfo {
    /// Create a new `ParseErrorInfo` with the given position and message.
    pub(crate) fn new(source_start: usize, message: &'static str) -> Self {
        ParseErrorInfo {
            source_start,
            message: Some(message),
        }
    }
}

impl core::fmt::Display for ParseErrorInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(msg) = self.message {
            write!(f, "{msg} at position {}", self.source_start)
        } else {
            write!(f, "parse error at position {}", self.source_start)
        }
    }
}

impl core::error::Error for ParseErrorInfo {}

/// Convert a `ParseError` to `ParseErrorInfo` for the execution chain.
///
/// Execution-phase errors lose specificity but retain position info.
/// The `reason` field is discarded.
impl From<ParseError> for ParseErrorInfo {
    fn from(err: ParseError) -> Self {
        match err {
            ParseError::UnbalancedQuote { pos } => ParseErrorInfo {
                source_start: pos,
                message: Some("unbalanced quote"),
            },
            ParseError::UnexpectedEof { pos } => ParseErrorInfo {
                source_start: pos,
                message: Some("unexpected end of input"),
            },
            ParseError::InvalidChar { pos, .. } => ParseErrorInfo {
                source_start: pos,
                message: Some("invalid character"),
            },
            ParseError::Reason { pos, reason } => ParseErrorInfo {
                source_start: pos,
                message: Some(reason),
            },
        }
    }
}

/// Convert an `i32` error to `ParseErrorInfo`.
///
/// This is a lossy conversion — position information is lost and defaults to 0.
/// Used for execution-phase errors that propagate through the call chain
/// (run_one, loop_, if_exec) where the original parse position is not available.
pub(crate) fn to_parse_err(_: i32) -> ParseErrorInfo {
    ParseErrorInfo {
        source_start: 0,
        message: None,
    }
}

impl From<i32> for ParseErrorInfo {
    fn from(_: i32) -> Self {
        ParseErrorInfo {
            source_start: 0,
            message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_parse_err_defaults_to_zero() {
        let info = to_parse_err(42);
        assert_eq!(info.source_start, 0);
    }

    #[test]
    fn parse_error_to_info_unbalanced_quote() {
        let err = ParseError::UnbalancedQuote { pos: 5 };
        let info: ParseErrorInfo = err.into();
        assert_eq!(info.source_start, 5);
        assert_eq!(info.message, Some("unbalanced quote"));
    }

    #[test]
    fn parse_error_to_info_reason() {
        let err = ParseError::Reason {
            pos: 10,
            reason: "syscall error",
        };
        let info: ParseErrorInfo = err.into();
        assert_eq!(info.source_start, 10);
        assert_eq!(info.message, Some("syscall error"));
    }
}

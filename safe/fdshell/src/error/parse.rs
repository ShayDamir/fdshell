#![forbid(unsafe_code)]

//! Parser errors (parse/*.rs).
//!
//! `ParseError` covers parse-time errors without position info.
//! Position is attached separately via `ParsePosition` on the `Report`.

use displaydoc::Display;
use error_stack::Report;

/// [ParseError] Parser errors
// Debug needed for impl Error (trait bound), not for display.
#[derive(Display, Debug)]
pub(crate) enum ParseError {
    /// unmatched quote
    UnbalancedQuote,
    /// unexpected end of input
    UnexpectedEof,
    /// NUL byte
    InvalidChar { ch: u8 },
    /// {reason}
    Reason { reason: &'static str },
}

impl std::error::Error for ParseError {}

/// Convert `i32` (errno) to `ParseError`.
///
/// `ShortCStr::push` returns `EINVAL` for NUL bytes.
impl From<i32> for ParseError {
    fn from(_: i32) -> Self {
        ParseError::InvalidChar { ch: 0 }
    }
}

/// Create a `Report<ParseError>` with `ParsePosition` attached.
pub(crate) fn report_error(reason: &'static str, pos: usize) -> Report<ParseError> {
    Report::new(ParseError::Reason { reason }).attach_opaque(ParsePosition { pos, input: None })
}

/// Create a `Report<ParseError>` for an unbalanced quote at `pos`.
pub(crate) fn report_unbalanced_quote(line: &[u8], pos: usize) -> Report<ParseError> {
    Report::new(ParseError::UnbalancedQuote).attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

/// Create a `Report<ParseError>` for unexpected EOF starting at `pos`.
pub(crate) fn report_unexpected_eof(line: &[u8], pos: usize) -> Report<ParseError> {
    Report::new(ParseError::UnexpectedEof).attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

/// Create a `Report<ParseError>` for an invalid character at `pos`.
pub(crate) fn report_invalid_char(ch: u8, pos: usize) -> Report<ParseError> {
    Report::new(ParseError::InvalidChar { ch }).attach_opaque(ParsePosition { pos, input: None })
}

/// Attached with the parse error to show position in error output.
#[derive(Debug)]
pub(crate) struct ParsePosition {
    /// Byte offset of the error in the input.
    pub(crate) pos: usize,
    /// The input line for formatting.
    pub(crate) input: Option<Vec<u8>>,
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

/// Convert a `ParseError` to `ParseErrorInfo` for the legacy execution chain.
impl From<ParseError> for ParseErrorInfo {
    fn from(err: ParseError) -> Self {
        match err {
            ParseError::UnbalancedQuote => ParseErrorInfo {
                source_start: 0,
                message: Some("unbalanced quote"),
            },
            ParseError::UnexpectedEof => ParseErrorInfo {
                source_start: 0,
                message: Some("unexpected end of input"),
            },
            ParseError::InvalidChar { .. } => ParseErrorInfo {
                source_start: 0,
                message: Some("invalid character"),
            },
            ParseError::Reason { reason } => ParseErrorInfo {
                source_start: 0,
                message: Some(reason),
            },
        }
    }
}

/// Convert an `i32` error to `ParseErrorInfo`.
///
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
    fn parse_error_to_info_defaults() {
        let err = ParseError::UnbalancedQuote;
        let info: ParseErrorInfo = err.into();
        assert_eq!(info.source_start, 0);
        assert_eq!(info.message, Some("unbalanced quote"));
    }

    #[test]
    fn parse_error_to_info_reason() {
        let err = ParseError::Reason {
            reason: "syscall error",
        };
        let info: ParseErrorInfo = err.into();
        assert_eq!(info.source_start, 0);
        assert_eq!(info.message, Some("syscall error"));
    }
}

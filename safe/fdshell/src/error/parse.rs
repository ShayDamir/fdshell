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

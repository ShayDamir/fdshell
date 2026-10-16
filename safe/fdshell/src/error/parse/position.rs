//! Position info carried on a `Report<ParseError>` (the `report_*` ctors and
//! the `ParsePosition` attachment of `error/parse.rs`).

use super::ParseError;
use alloc::vec::Vec;
use error_stack::Report;

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

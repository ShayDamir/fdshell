//! Position info carried on a `Report<ParseError>` (the `report_*` ctors and
//! the `ParsePosition` attachment of `error/parse.rs`).

use super::ParseError;
use crate::keywords::{Closer, Keyword, keyword_kind, keyword_len, time_prefix_len};
use alloc::vec::Vec;
use builtins::error::Suggestion;
use error_stack::Report;
use sys::ShortCStr;

/// Create a `Report<ParseError>` for an unbalanced quote at `pos`.
pub(crate) fn report_unbalanced_quote(line: &[u8], pos: usize) -> Report<ParseError> {
    Report::new(ParseError::UnbalancedQuote).attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

/// Create a `Report<ParseError>` for a `)` token at `pos` that a `case` arm's
/// command list cannot take (task #419 — the position ride from the arm walk).
pub(crate) fn report_case_arm_close_paren(line: &[u8], pos: usize) -> Report<ParseError> {
    Report::new(ParseError::CaseArmUnexpectedCloseParen).attach_opaque(ParsePosition {
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

/// Create a `Report<ParseError>` for a **stray closing keyword word** at `pos`
/// (task #436): the word stands in command position and closes no open block of
/// its kind, which bash's grammar refuses before running anything. The caret sits
/// on the word (the mechanism of [`report_unbalanced_quote`]) and the
/// [`Suggestion`] names the blocks that closer serves (STYLE §4.8).
pub(crate) fn report_stray_closer(line: &[u8], pos: usize) -> Report<ParseError> {
    let rest = line.get(pos..).unwrap_or(b"");
    // A `time [-p]` prefix written in front of the closer is transparent to the
    // recognition table, so the caret and the message name the keyword behind it.
    let skip = time_prefix_len(rest).unwrap_or(0);
    let rest = rest.get(skip..).unwrap_or(b"");
    let word = rest.get(..keyword_len(rest)).unwrap_or(b"");
    // The word is the closer itself; the **one** recognition table names the
    // blocks it serves, so the suggestion cannot drift from the pairing.
    let Ok(closer) = ShortCStr::from_vec(word.to_vec()) else {
        return Report::new(ParseError::Never);
    };
    let suggestion = match keyword_kind(word, line, pos + skip + word.len()) {
        Some(Keyword::Closes(Closer::Fi)) => {
            "a stray 'fi' needs an open `if` block: open one, or drop the word"
        }
        Some(Keyword::Closes(Closer::Done)) => {
            "a stray 'done' needs an open `while`/`until`/`for` block: open one, or drop the word"
        }
        Some(Keyword::Closes(Closer::Esac)) => {
            "a stray 'esac' needs an open `case` block: open one, or drop the word"
        }
        Some(Keyword::Closes(Closer::Brace)) => {
            "a stray '}' needs an open '{ …; }` group: open one, or drop the word"
        }
        // The scan marks only a word the recognition table calls a closer, so this
        // arm answers a line the marker never named; keep it total without a
        // panic (STYLE §4.10).
        _ => "write a closing keyword only where a block of its own kind is open",
    };
    Report::new(ParseError::StrayCloser { closer })
        .attach_opaque(Suggestion(suggestion))
        .attach_opaque(ParsePosition {
            pos: pos + skip,
            input: Some(line.to_vec()),
        })
}

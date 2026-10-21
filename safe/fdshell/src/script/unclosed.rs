//! The report for a block whose closing keyword is missing (task #277).
//!
//! The statement layer owns this failure, so it must be actionable
//! (STYLE §4.7): the message names the opener and the keyword it wants, and the
//! `ParsePosition` attachment draws the caret under the opener — the bare
//! `CmdError::Parse` names nothing. The exit code stays the shell-wide 1 of a
//! parse error (the rc-1-vs-2 family, README).

use core::ffi::CStr;
use error_stack::Report;

use crate::error::cmd::CmdError;
use crate::error::parse::{ParseError, ParsePosition};
use crate::keywords::{leading_ws, time_prefix_len};
use sys::ShortCStr;

/// The report an unclosed block returns: [`parse_error`]'s message names the
/// construct, carried in the statement layer's `CmdError::Parse` class.
pub(super) fn report(line: &[u8], start: usize) -> Report<CmdError> {
    parse_error(line, start).change_context(CmdError::Parse)
}

/// The unclosed-block [`ParseError`], with the caret attached at the opener
/// (the mechanism of the `unmatched quote` report). Split from [`report`] so the
/// unit test asserts the variant itself, not only its context.
pub(super) fn parse_error(line: &[u8], start: usize) -> Report<ParseError> {
    let (off, word) = opener_words(line.get(start..).unwrap_or(b""));
    // Two invariant breaches, both impossible on a validated `ScriptText`
    // (task #275): a NUL in the opener word, and an opener offset past the
    // line. An invariant violation is never relabeled as a domain error the
    // user can fix (STYLE §4.17) — the shape of `parse/heredoc::unterminated`.
    let (Ok(opener), Some(pos)) = (ShortCStr::from_vec(word.to_vec()), start.checked_add(off))
    else {
        return Report::new(ParseError::Never);
    };
    Report::new(ParseError::MissingBlockCloser {
        opener,
        closer: ShortCStr::from(closer(word)),
    })
    .attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

/// The opener word a block is named by, with its offset in `rest`: leading
/// whitespace skipped, the transparent `time [-p]` prefix run skipped
/// (`time while …` is named `while`), then the word up to whitespace, `;` or the
/// `{` that opens the body (`f() {` is named `f()`, `if true;` is named `if`).
/// The first byte is never a terminator, so a group run `{ …` names `{`.
fn opener_words(rest: &[u8]) -> (usize, &[u8]) {
    let lead = leading_ws(rest);
    let body = rest.get(lead..).unwrap_or(b"");
    let off = lead + time_prefix_len(body).unwrap_or(0);
    let word = rest.get(off..).unwrap_or(b"");
    let end = word
        .iter()
        .skip(1)
        .position(|&b| b.is_ascii_whitespace() || matches!(b, b';' | b'{'))
        .map_or(word.len(), |p| p + 1);
    (off, word.get(..end).unwrap_or(b""))
}

/// The keyword that closes a block opened by the word `word`. A `{` group, a
/// `function` keyword definition, a `name()` definition and an event-`wait`
/// block all want `}`.
fn closer(word: &[u8]) -> &'static CStr {
    match word {
        b"if" => c"fi",
        b"for" | b"while" | b"until" => c"done",
        b"case" => c"esac",
        _ => c"}",
    }
}

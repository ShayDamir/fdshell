//! Attach a token slice's claimed heredoc bodies to its verbatim text. In
//! **append** mode (a single-line block: the bodies sit past the closing
//! keyword, so the slice's text stops at its last token) the body bytes are
//! appended after a newline, as `cond/part.rs::reconstruct` does for cond-list
//! parts; in **extension** mode they are interleaved with the slice's own
//! lines and [`owned_span`] covers them.

use super::Token;
use super::cond_bodies::{claimed_bodies, claimed_regions};
use super::semi::token_range;
use crate::cond::part::reconstruct;
use crate::error::parse::ParseError;
use crate::scan::heredoc::terminator_spans;
use error_stack::{Report, ResultExt};
use sys::ScriptText;

/// The verbatim text of `tokens`, with the body bytes its `<<` operators claim
/// appended when they sit past the slice (the same append/extension
/// discriminator the block scanner applies to the whole block).
pub(crate) fn part_text(
    text: &ScriptText,
    tokens: &[Token],
) -> Result<ScriptText, Report<ParseError>> {
    let (start, end) = token_range(tokens);
    // The block scanner's discriminator at arm granularity: regions that start
    // at or past the slice's end are outside its text and must be appended.
    // Below that (extension mode, and the no-operator case) they are inline.
    let regions = claimed_regions(text, tokens);
    if regions.first().is_none_or(|&(s, _)| s < end) {
        return owned_span(text, tokens);
    }
    let bodies = claimed_bodies(text, tokens);
    let line = text.as_bytes().change_context(ParseError::Never)?;
    reconstruct(text, line, start, end, &bodies).change_context(ParseError::Never)
}

/// The slice's verbatim text, widened to the end of every **terminator line** it
/// reaches — inline regions are part of the text, and the plain token span stops
/// at the last token's end. A terminator line whose word sits mid-line (its
/// syntax bytes are blank for the tokenizer, LESSONS "A heredoc terminator line
/// is structure, not command bytes") leaves its tail bytes behind every token, so
/// `cat <<$(x)` + body + `$(x)` in a block body would reach the byte scan with
/// its terminator truncated to `$(x` and never match the delimiter word. A
/// terminator ending in a word byte is already covered, so the widening is a
/// no-op there, and the newline behind the terminator stays out of the slice
/// exactly as it does on a plain word terminator (the `;` it tokenizes as drops
/// out of the body).
fn owned_span(text: &ScriptText, tokens: &[Token]) -> Result<ScriptText, Report<ParseError>> {
    let (start, end) = token_range(tokens);
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let reach = terminator_spans(line)
        .into_iter()
        .filter(|&(s, _)| s >= start && s < end)
        .map(|(_, e)| e)
        .max()
        .unwrap_or(end);
    let end = end.max(reach);
    text.subslice(start, end.saturating_sub(start))
        .ok_or(ParseError::Never.into())
}

#[cfg(test)]
mod tests;

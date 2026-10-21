//! Attach a token slice's claimed heredoc bodies to its verbatim text. In
//! **append** mode (a single-line block: the bodies sit past the closing
//! keyword, so the slice's text stops at its last token) the body bytes are
//! appended after a newline, as `cond/part.rs::reconstruct` does for cond-list
//! parts; in **extension** mode they are interleaved with the slice's own
//! lines and `verbatim` already covers them.

use super::Token;
use super::cond_bodies::{claimed_bodies, claimed_regions};
use super::semi::{token_range, verbatim};
use crate::cond::part::reconstruct;
use crate::error::parse::ParseError;
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
        return verbatim(text, tokens);
    }
    let bodies = claimed_bodies(text, tokens);
    let line = text.as_bytes().change_context(ParseError::Never)?;
    reconstruct(text, line, start, end, &bodies).change_context(ParseError::Never)
}

#[cfg(test)]
mod tests;

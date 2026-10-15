//! Attach a token slice's claimed heredoc body regions to its verbatim text.
//! In **append** mode (a single-line block: the bodies sit past the closing
//! keyword, so the slice's text stops at its last token) the regions are
//! appended after a newline, as `cond/part.rs::reconstruct` does for cond-list
//! parts; in **extension** mode they are interleaved with the slice's own
//! lines and `verbatim` already covers them.

use super::Token;
use super::cond_bodies::claimed_regions;
use super::semi::{token_range, verbatim};
use crate::cond::part::reconstruct;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;

/// The verbatim text of `tokens`, with the body regions its `<<` operators
/// claim appended when they sit past the slice (the same append/extension
/// discriminator the block scanner applies to the whole block).
pub(crate) fn part_text(
    text: &ScriptText,
    tokens: &[Token],
) -> Result<ScriptText, Report<ParseError>> {
    let (start, end) = token_range(tokens);
    let regions = claimed_regions(text, tokens);
    // Extension mode, and the no-operator case: the bodies are already inline.
    if regions.first().is_none_or(|&(s, _)| s < end) {
        return verbatim(text, tokens);
    }
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let bodies: Vec<Vec<u8>> = regions
        .into_iter()
        .map(|(s, e)| line.get(s..e).unwrap_or(b"").to_vec())
        .collect();
    reconstruct(text, line, start, end, &bodies).change_context(ParseError::Never)
}

#[cfg(test)]
mod tests;

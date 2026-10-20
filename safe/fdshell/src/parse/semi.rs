use super::Token;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ScriptText;
use sys::ShortCStr;

mod closing;
mod preceded;
mod trim;

pub(crate) use closing::{List, closing_keyword_index, list_closing_keyword_index, up_to_closing};
pub(crate) use preceded::{Term, amp_semi, find_preceded_by, is_list_terminator};
pub(crate) use trim::{block_body, body_end, trim_body, trim_condition, trim_semi};

/// The joined word of `tokens` **with its mask and zero-byte span list**: the
/// space inserted between two tokens takes a `false` bit and every token byte
/// keeps its own, so the mask stays parallel to the text (task #269: a joined
/// word that is substituted later needs the mask of the whole join). Spans
/// travel with the mask (task #321): each token's span indices shift by the
/// join offset accumulated before it. Call sites take the join's **text** as
/// `try_join_masked(tokens).0` (`command_word`, `case_block`), so the
/// space-insertion rule lives in exactly one place — the word, its mask and
/// its spans can never drift apart.
pub(crate) fn try_join_masked(tokens: &[Token]) -> (ShortCStr, Vec<bool>, Vec<usize>) {
    let mut out = ShortCStr::new();
    let mut mask = Vec::new();
    let mut spans = Vec::new();
    for (t, _, _, _, t_mask, t_spans) in tokens {
        let offset = out.len();
        if !out.is_empty() {
            out.push(c" ");
            mask.push(false);
        }
        out.push(t);
        mask.extend(t_mask.iter().copied());
        spans.extend(t_spans.iter().map(|&i| offset + i));
    }
    (out, mask, spans)
}

/// A verbatim subslice of `text` covering the given tokens' byte ranges.
pub(crate) fn verbatim(
    text: &ScriptText,
    tokens: &[Token],
) -> Result<ScriptText, Report<ParseError>> {
    let (off, end) = token_range(tokens);
    let t = text.subslice(off, end - off).ok_or(ParseError::Never)?;
    Ok(t)
}

/// Byte range `(start, end)` covered by the first and last tokens, or `(0, 0)`.
pub(crate) fn token_range(tokens: &[Token]) -> (usize, usize) {
    match (tokens.first(), tokens.last()) {
        (Some(f), Some(l)) => (f.1, l.2),
        _ => (0, 0),
    }
}

#[cfg(test)]
mod tests;

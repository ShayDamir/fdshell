use super::Token;
use super::word_quoted::keyword_word;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ScriptText;
use sys::ShortCStr;

mod closing;

pub(crate) use closing::{List, closing_keyword_index, list_closing_keyword_index, up_to_closing};

/// The `;`-preceded keyword word after `start`: `needle` is a keyword only
/// when it carries no quote byte ([`keyword_word`], task #247), so a quoted
/// `then`/`do`/`else` is a command word and the block reports its own missing
/// keyword (bash: a syntax error).
pub(crate) fn find_preceded_by_semi(
    tokens: &[Token],
    start: usize,
    needle: &[u8],
    line: &[u8],
) -> Option<usize> {
    for (i, tok) in tokens.iter().enumerate().skip(start) {
        let preceded = i > 0
            && tokens
                .get(i - 1)
                .is_some_and(|(p, _, _, _, _, _)| p.eq_bytes(b";"));
        if keyword_word(tok, line, needle) && preceded {
            return Some(i);
        }
    }
    None
}

pub(crate) fn trim_semi(tokens: &[Token]) -> &[Token] {
    let start = tokens
        .iter()
        .take_while(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens
        .iter()
        .rev()
        .take_while(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
        .count();
    let end = tokens.len().saturating_sub(end);
    tokens.get(start..end).unwrap_or(&[])
}

/// A block arm's body tokens: `tokens` with the `;`s bracketing it trimmed,
/// `err` when nothing is left — bash rejects an empty body (`if x; then; fi`,
/// `while x; do; done`, `for i in a; do; done`: all rc 2, and an empty `do`
/// body makes the loop unbounded at run time).
pub(crate) fn block_body(
    tokens: &[Token],
    err: ParseError,
) -> Result<&[Token], Report<ParseError>> {
    let body = trim_semi(tokens);
    ensure!(!body.is_empty(), err);
    Ok(body)
}

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

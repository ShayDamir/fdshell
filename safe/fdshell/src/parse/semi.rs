use super::Token;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ScriptText;
use sys::ShortCStr;

mod closing;
mod preceded;

pub(crate) use closing::{List, closing_keyword_index, list_closing_keyword_index, up_to_closing};
pub(crate) use preceded::{Term, find_preceded_by, is_list_terminator};

/// The exclusive end of a body slice written **before** the closer at
/// `closer_idx`: POSIX's `;` there is dropped (the body text ends at its last
/// command), a plain-`&` terminator is **kept** — the body's own scan turns it
/// into a background job (task #354) — and any other word the closer is glued to
/// stays in the body, as `if_block` applies today.
pub(crate) fn body_end(tokens: &[Token], closer_idx: usize) -> usize {
    let glued = tokens
        .get(closer_idx.saturating_sub(1))
        .is_some_and(|(t, _, _, _, _, _)| !t.eq_bytes(b";"));
    if glued {
        closer_idx
    } else {
        closer_idx.saturating_sub(1)
    }
}

/// The body tokens with the `;`s bracketing them trimmed: the `;` a shell
/// separator leaves around a body is not a word of the body.
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

/// A **condition** slice: on top of [`trim_semi`], the plain `&` that precedes a
/// clause keyword terminates the compound list, and the condition is run as its
/// own command list (its scanner never saw the `&`, so nothing here consumes it):
/// `if true & then …` runs `true` as the condition (task #354; the background
/// job of a `&`-terminated condition is #356's).
pub(crate) fn trim_condition(tokens: &[Token]) -> &[Token] {
    let body = trim_semi(tokens);
    let amp = usize::from(
        body.last()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"&")),
    );
    let end = body.len().saturating_sub(amp);
    body.get(..end).unwrap_or(&[])
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

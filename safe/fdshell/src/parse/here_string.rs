use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::parse::Token;
use crate::redirect::RedirectDef;
use crate::scan::heredoc::{io_fd, operator_word, string_span};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// Parse a here-string token at `i`: `[N]<<<word`, or a bare `[N]<<<` taking
/// the next token as its word. The operator shape is read from the token's
/// **raw span** through the shared `string_span`, never from the token text:
/// quote removal makes `3"<<<"x` read exactly like `3<<<x`, and a token-text
/// rule for a raw-byte structure is the byte/token count mismatch (LESSONS,
/// task #276's `<<` twin). Returns the redirect (the word with **its own**
/// quote mask, task #269, on the `N<<<word` descriptor — 0 for the unprefixed
/// form) and how many following tokens the operator consumes (1 for the bare
/// form). The operator-quoting rule (task #264: a redirect is made of
/// **unquoted** bytes) is *structural* here, the byte-level twin of the
/// token-level `redirect::quoted_bytes` gate: `string_span` matches the raw
/// span, so a leading quote byte kills the io-number (a digit must begin the
/// span) and a quote byte between the `<`s kills the `<<<`. That is why
/// `cat 3'<<<'x`, `cat 3"<<<"x` and every fully quoted word are word data at
/// both levels.
pub fn parse_here_string(
    line: &[u8],
    tokens: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let Some((t, start, end, _fq, mask, spans)) = tokens.get(i) else {
        return Ok(None);
    };
    let Some((op, prefix)) = string_span(line, *start, *end) else {
        return Ok(None);
    };
    // `<<<""` is one word whose quoted part contributes no bytes: its word is
    // the empty quoted region, not the next token. A bare `<<<` takes the next
    // token as its word; at end of input the word is empty.
    let off = prefix + 3;
    let (word, mask, spans, extra) = if has_attached_word(t.len(), prefix) {
        (
            t.get(off..).ok_or(ParseError::Never)?.clone(),
            word_mask(mask, off),
            word_spans(spans, off),
            0,
        )
    } else if super::word_quoted::word_quoted(t, *start, *end) {
        (ShortCStr::new(), Vec::new(), Vec::new(), 0)
    } else {
        match tokens.get(i + 1) {
            Some(next) if operator_word(&next.0, &next.4) => bail!(ParseError::InvalidRedirect),
            Some(next) => (next.0.clone(), next.4.clone(), next.5.clone(), 1),
            None => (ShortCStr::new(), Vec::new(), Vec::new(), 0),
        }
    };
    let digits = line.get(*start..op).ok_or(ParseError::Never)?;
    let Some(export_to) = io_fd(digits) else {
        let prefix = ShortCStr::from_vec(digits.to_vec()).change_context(ParseError::Never)?;
        bail!(ParseError::InvalidHereStringFd { prefix });
    };
    Ok(Some((
        RedirectDef::here_string(export_to, word, mask, spans),
        extra,
    )))
}

/// The attached word's mask: the operator token's mask sliced at the **word's
/// offset** inside the token. For a here-string that offset is `prefix + 3`
/// (the `[io-number] <<<` operator); a path redirect passes its own offset one
/// past the operator's last byte (`parse/redirect/path_word.rs`, task #278).
/// This is the only slicer of a token mask for an attached word, so the word
/// and its mask cannot drift apart (LESSONS: slice the word, its mask and its
/// quoted flags with the same offset). The here-string's prefix digits are raw
/// bytes by construction — `string_span` only reports a span whose digits
/// begin it — so the token text's first `prefix + 3` bytes are 1:1 with the
/// mask. A mask shorter than the word degrades to unquoted, the same padding
/// `substitute/arg.rs` applies; the empty word of `<<<""` keeps the empty mask,
/// which `substitute::fully_quoted` reads as quoted.
pub(crate) fn word_mask(op_mask: &[MaskBit], offset: usize) -> Vec<MaskBit> {
    op_mask.get(offset..).unwrap_or_default().to_vec()
}

/// The attached word's zero-byte quote spans (task #321): sliced at the same
/// `offset` as [`word_mask`] and shifted down to word indices.
pub(crate) fn word_spans(op_spans: &[usize], offset: usize) -> Vec<usize> {
    op_spans
        .iter()
        .filter(|&&i| i >= offset)
        .map(|&i| i - offset)
        .collect()
}

/// Whether the operator token carries its own word: the token is longer than
/// its `prefix + 3` operator bytes (`3<<<x`, `3<<<""`), as opposed to the bare
/// `3<<<` form, which takes the next token. One owner for the test, so the
/// parse and the brace-protection consumer cannot drift.
fn has_attached_word(len: usize, prefix: usize) -> bool {
    len > prefix + 3
}

/// The token index of every here-string word: the operator token in the
/// attached form (`<<<word`, `3<<<word`, `<<<""`) and the following token in
/// the bare `<<<` form.
pub(crate) fn word_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (i, (t, start, end, _fq, _mask, _spans)) in tokens.iter().enumerate() {
        let Some((_op, prefix)) = string_span(line, *start, *end) else {
            continue;
        };
        if has_attached_word(t.len(), prefix) || super::word_quoted::word_quoted(t, *start, *end) {
            out.push(i);
        } else if tokens.get(i + 1).is_some() {
            out.push(i + 1);
        }
    }
    out
}

#[cfg(test)]
mod tests;

//! Group-separation rules of the gobbler: what counts as a `..` sequence
//! start, when a top-level `{` is ignored, and how a `$(…)` span is skipped.

use crate::paren_scan::scan_paren_body;

/// Whether `..` starts at `i` and can begin a sequence (next byte is not `}`).
pub(super) fn seq_sep(text: &[u8], i: usize) -> bool {
    match text.get(i..i + 2) {
        Some(s) if s == (b"..").as_slice() => text.get(i + 2) != Some(&b'}'),
        _ => false,
    }
}

/// The bash rule: a top-level `{` is ignored when preceded by word start or
/// whitespace and followed by whitespace or `}` (so `{a, b}` is not a group).
pub(super) fn ignore_open_brace(text: &[u8], i: usize) -> bool {
    let prev_blank = i == 0 || text.get(i - 1).is_some_and(|&b| is_blank(b));
    let next = text.get(i + 1);
    prev_blank && (next.is_none_or(|&b| is_blank(b)) || next == Some(&b'}'))
}

fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n')
}

/// Skip a `$(…)` span starting at `i`; return the index just past the
/// matching `)`. Reuses the tokenizer's scanner for exact parity. The caller
/// just checked the `(` at `i + 1`, so `rest` exists (possibly empty); an
/// unterminated span scans to the end of the word and swallows the rest.
pub(super) fn skip_dollar_paren(text: &[u8], i: usize) -> usize {
    let rest = text.get(i + 2..).unwrap_or_default();
    match scan_paren_body(&mut rest.iter().copied().peekable(), 1) {
        // `$(` + body + `)`: the span is `3 + body.len()` bytes long.
        Some(body) => i + 3 + body.len(),
        None => text.len(),
    }
}

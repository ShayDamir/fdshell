//! Word and line helpers for heredoc (`<<`) detection.

use super::super::ScanState;
pub(crate) use word::word_span;

mod word;

/// A byte that ends a token word for the tokenizer (the split set of
/// `token/step.rs`; `&`, `<`, `>` and `#` are ordinary word bytes).
pub(crate) fn token_word_end(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b';' | b'\n' | b'|' | b')')
}

/// The raw span of the delimiter word of the `<<` operator whose first byte
/// is two before `j`: the attached `<<WORD` span, or — for a bare `<<` — the
/// next word after whitespace. A quoted span is one word (see [`word::word_span`]),
/// and the bytes are returned raw: quote removal is `op::Operator::new`'s, so
/// both levels apply it once. Returns the span end and raw bytes; `None`
/// when a bare `<<` has no delimiter word (the parser reports that).
pub(crate) fn delimiter_span(line: &[u8], mut j: usize, to: usize) -> Option<(usize, &[u8])> {
    let attached = j;
    while j < to && !token_word_end(line.get(j).copied().unwrap_or(0)) {
        j = word_span(line, j, to);
    }
    if j > attached {
        return Some((j, line.get(attached..j)?));
    }
    loop {
        // Bound first, body second — exactly the old loop's order (the bound
        // is a method call on the exact range: no operator site, rule 18/24,
        // task #513).
        if (j..to).is_empty() {
            break;
        }
        if !matches!(line.get(j).copied(), Some(b' ') | Some(b'\t')) {
            break;
        }
        j += 1;
    }
    let word = j;
    while j < to && !token_word_end(line.get(j).copied().unwrap_or(0)) {
        j = word_span(line, j, to);
    }
    // `word..j` is within `from..to` by construction.
    let raw = line.get(word..j)?;
    if raw.is_empty()
        || matches!(
            raw.first().copied(),
            Some(b'<') | Some(b'>') | Some(b'&') | Some(b'%')
        )
    {
        return None;
    }
    Some((j, raw))
}

/// The index just after the first newline that is not inside quotes, a
/// single-quoted span, backticks, or a `$( )` substitution; `None` when the
/// line has none.
pub(crate) fn first_unquoted_newline(line: &[u8]) -> Option<usize> {
    let mut state = ScanState::new();
    let mut i = 0;
    // The walk ends through the range's own `is_empty` (no operator site —
    // rule 18/24, task #513); the body is byte-for-byte the old loop's, and
    // `advance` never steps past `line.len()`, so the old bound and the new
    // terminator answer on the same index.
    loop {
        if (i..line.len()).is_empty() {
            break;
        }
        let bare =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
        if bare && line.get(i) == Some(&b'\n') {
            return Some(i + 1);
        }
        i = state.advance(line, i);
    }
    None
}

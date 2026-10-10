//! The ONE `>`-terminated operator rule behind the `>|` clobber operator
//! (POSIX #2.2), shared by the tokenizer's `|` absorption
//! (`parse/token_pipe.rs`) and the byte-level heredoc word scan
//! (`scan/heredoc/ops.rs`): the two layers decide raw-byte structure from the
//! same predicate over the word's raw bytes + quote mask, so their `<<` counts
//! cannot diverge (LESSONS: byte-level and token-level rules must agree).

use alloc::vec::Vec;

/// The shared rule: the word's last byte is `>`, unquoted and not the second
/// byte of an escape pair, and no unquoted `>`/`<` byte comes before it — so
/// `a>`, `2>|`, `&>` and the bare `>` are `>`-terminated, while `>>`, `>&`,
/// `<>`, `a>b` and `x\>` are not. A `%`-leading word (the capture operator)
/// keeps its own absorption, where earlier operator bytes do not disqualify it.
/// `quoted[i]` is true when byte `i` is inside double quotes, so a quoted `>`
/// never terminates an operator word.
pub(crate) fn clobber_word(bytes: &[u8], quoted: &[bool]) -> bool {
    let last = bytes.len().saturating_sub(1);
    if bytes.get(last) != Some(&b'>') || quoted.get(last) == Some(&true) {
        return false;
    }
    let percent = bytes.first() == Some(&b'%');
    let mut i = 0;
    while i < last {
        let quoted_byte = quoted.get(i) == Some(&true);
        if !quoted_byte && bytes.get(i) == Some(&b'\\') {
            // The escape pair shields the byte after the backslash.
            i += 2;
            continue;
        }
        if !quoted_byte && !percent && matches!(bytes.get(i), Some(b'>' | b'<')) {
            return false;
        }
        i += 1;
    }
    // Landing past `last` means the `>` was an escape pair's second byte.
    i == last
}

/// Per-byte double-quote mask of `line[0..e]`: `true` for a byte inside `"…"`.
/// The quote delimiters and the bytes of an unquoted escape pair are `false`,
/// and the mask is parallel to the bytes. It is scanned from the line start, so
/// a word's mask slice knows the quote state at its first byte (a word that
/// begins with the closing `"` of a quoted span is the case that a word-local
/// scan gets wrong).
pub(crate) fn quote_mask(line: &[u8], e: usize) -> Vec<bool> {
    let mut mask = Vec::new();
    let mut in_quote = false;
    let mut i = 0;
    while i < e {
        mask.push(in_quote);
        match line.get(i) {
            Some(b'"') => {
                in_quote = !in_quote;
                i += 1;
            }
            Some(b'\\') if !in_quote => {
                mask.push(false);
                i += 2;
            }
            _ => i += 1,
        }
    }
    mask
}

/// Whether the `|` at `k` is the byte of a `>|` clobber operator, so it is an
/// operator byte, not a pipeline pipe, and it does not break a word. Callers
/// pass a `k` they have already matched to a `|`; the word is the run that ends
/// at `k - 1`, back to a byte that ends a word: whitespace, `;`, newline, or
/// `)` — `scan::advance::is_word_break` without `|`, `<` and `>`, because an
/// absorbed `|` is part of the operator word and operator bytes never break a
/// word (as in the tokenizer).
pub(crate) fn clobber_pipe(line: &[u8], k: usize) -> bool {
    let mask = quote_mask(line, k);
    let mut s = k;
    while s > 0
        && line
            .get(s - 1)
            .is_some_and(|&b| !matches!(b, b' ' | b'\t' | b';' | b'\n' | b')'))
    {
        s -= 1;
    }
    let word = line.get(s..k).unwrap_or(b"");
    clobber_word(word, mask.get(s..k).unwrap_or(&[]))
}

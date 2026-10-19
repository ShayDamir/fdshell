//! The ONE pipeline-pipe rule: which `|` byte is the tokenizer's pipe token.
//!
//! The tokenizer emits a pipe token for an unquoted, unescaped `|` that its
//! own word is not absorbing (`parse::token::step`'s `|` arm +
//! `parse::token_pipe::pipe_token`, which folds a `|` into a `>`-terminated
//! word). Every byte-level reader of pipeline structure must apply exactly
//! those three tests to the raw bytes, never to a token text: a quoted `"|"`
//! and an escaped `\|` are word content whose *text* is `|`, and a token-text
//! test reads them as pipeline stages (LESSONS: the byte-level and
//! token-level counts must agree).

use super::{clobber_pipe, quote_mask};

/// The `|` at `k` is the tokenizer's pipe-token byte: it sits outside every
/// quote span, it is not the tail of an unquoted `\X` escape pair, and it is
/// not absorbed into a `>`-terminated operator word (`clobber_pipe`: `>|`,
/// `&>`, `%>`). A quoted/escaped `|` is word content, so the answer is `false`
/// before the `clobber_pipe` word walk is ever needed.
pub(crate) fn pipeline_pipe(line: &[u8], k: usize) -> bool {
    let mask = quote_mask(line, k + 1);
    if mask.get(k) == Some(&true) {
        return false;
    }
    // `clobber_word`'s escape-pair walk over the same word: a walk that jumps
    // past `k` read `k` as a pair's tail (`x\|` is one word, no pipe in it).
    let mut i = word_start(line, k);
    while i < k {
        let escapes = mask.get(i) != Some(&true) && line.get(i) == Some(&b'\\');
        i += 1 + usize::from(escapes);
    }
    i == k && !clobber_pipe(line, k)
}

/// The token whose raw span is `line[start..end]` **is** the tokenizer's pipe
/// token: a pipe token's span is exactly one `|` byte (`parse::token::step`'s
/// `|` arm pushes `pos - 1..pos`). Every token-layer pipeline split reads this,
/// never the token text: quote removal makes a `"|"`, a `'|'` and an escaped
/// `\|` word tokens whose *text* is `|`, and a text test reads them as stage
/// boundaries (`echo "|" <<EOF` is one command, `case x in "|")` is one
/// pattern). The byte-level twin of this test is [`pipeline_pipe`].
pub(crate) fn pipe_token_span(line: &[u8], start: usize, end: usize) -> bool {
    line.get(start..end) == Some(&b"|"[..])
}

/// The word-start scan of [`clobber_pipe`]: the first byte of the word that
/// ends at `k`. Whitespace, `;`, newline and `)` end a word; operator bytes
/// never break one, because an absorbed `|` is part of the operator word.
pub(super) fn word_start(line: &[u8], k: usize) -> usize {
    let mut s = k;
    while s > 0
        && line
            .get(s - 1)
            .is_some_and(|&b| !matches!(b, b' ' | b'\t' | b';' | b'\n' | b')'))
    {
        s -= 1;
    }
    s
}

#[cfg(test)]
mod tests;

use super::Token;
use crate::keywords::quoted_word;
use sys::ShortCStr;

/// Whether the word's raw text (the byte span `(start, end)`) contained
/// quotes or escapes: the raw span is longer than the word's own bytes.
///
/// An empty quoted region contributes no bytes to the word (and its per-byte
/// mask), so "this word was quoted" is not representable in the mask — it has
/// to be derived from the span. The same test is true for escaped bytes
/// (`"a\"b"`), which is intended: bash keeps such a word whole when its
/// expansion is empty, too.
pub(crate) fn word_quoted(t: &ShortCStr, start: usize, end: usize) -> bool {
    end - start > t.len()
}

/// The keyword-recognition gate (POSIX §2.9.1, task #247): `tok` *is* the
/// keyword word `kw` only when the word carries no quote byte. Token-level
/// twin of the gate `keywords::keyword_delta` applies to block openers and
/// closers — both read [`quoted_word`] on the same raw span, so the
/// block-depth scan and the block builders cannot disagree. Not the per-byte
/// mask (`""if` has no bit) and not `fq` (`i"f"` is not fully quoted): bash
/// runs both as command words.
pub(crate) fn keyword_word(tok: &Token, line: &[u8], kw: &[u8]) -> bool {
    let (t, _start, end, _fq, _mask, _spans) = tok;
    t.eq_bytes(kw) && !quoted_word(line, *end)
}

#[cfg(test)]
mod tests;

//! Escape-pair folding: every unquoted `\X` pair of a byte slice folds to `X`
//! (POSIX #4.1 — the backslash is removed and the escaped byte loses its
//! special meaning), so a word/delimiter text is compared in its folded form.
//! Two entry points: [`fold`] reads the quote state off the raw bytes (a
//! delimiter text still carries its `"`), [`fold_mask`] reads it from a word's
//! quote mask (the tokenizer strips the quotes from word text).

use crate::bytes::QUOTE;
use alloc::vec::Vec;
use sys::{ShortCStr, ShortCStrError};

/// `raw` with every unquoted escape pair `\X` folded to `X`. A `\` inside a
/// double-quoted span keeps its backslash (POSIX #4.2), and a trailing `\` at
/// the end of `raw` keeps it.
pub(crate) fn fold(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut quoted = false;
    while let Some(&b) = raw.get(i) {
        if b == QUOTE {
            quoted = !quoted;
            out.push(b);
            i += 1;
        } else if b == b'\\' && !quoted {
            i += 1;
            match raw.get(i) {
                Some(&c) => out.push(c),
                None => out.push(b),
            }
            i += 1;
        } else {
            out.push(b);
            i += 1;
        }
    }
    out
}

/// `bytes` with every unquoted escape pair `\X` folded to `X`, and the quote
/// `mask` aligned to the folded text. The quote state comes from the mask (a
/// word's text has no `"` bytes), so a pair whose `\` bit is `true` keeps its
/// backslash (POSIX #4.2), the escaped byte of an unquoted pair comes out
/// protected (`true` — it never splits on IFS or globs), and a trailing `\`
/// keeps its backslash with its own mask bit.
pub(crate) fn fold_mask(bytes: &[u8], mask: &[bool]) -> (Vec<u8>, Vec<bool>) {
    let mut out = Vec::new();
    let mut out_mask = Vec::new();
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        let quoted = mask.get(i).copied().unwrap_or(false);
        match bytes.get(i + 1) {
            // Unquoted pair: drop the backslash, protect the escaped byte.
            Some(&c) if b == b'\\' && !quoted => {
                out.push(c);
                out_mask.push(true);
                i += 2;
            }
            _ => {
                out.push(b);
                out_mask.push(quoted);
                i += 1;
            }
        }
    }
    (out, out_mask)
}

/// [`fold_mask`] on a word, returning the folded word with its aligned mask.
pub(crate) fn fold_word(
    word: &ShortCStr,
    mask: &[bool],
) -> Result<(ShortCStr, Vec<bool>), ShortCStrError> {
    let (bytes, out_mask) = fold_mask(word.as_bytes()?, mask);
    Ok((ShortCStr::from_vec(bytes)?, out_mask))
}

#[cfg(test)]
mod tests;

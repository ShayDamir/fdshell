//! Escape-pair folding: every unquoted `\X` pair of a byte slice folds to `X`
//! (POSIX #4.1 — the backslash is removed and the escaped byte loses its
//! special meaning), so a word/delimiter text is compared in its folded form.
//! Three entry points, one state machine (`fold_impl`): [`fold`] reads the
//! quote state off the raw bytes and keeps the quote bytes, [`fold_quotes`]
//! reads the same raw bytes and applies POSIX quote removal on top of the
//! folding (every `"`/`'` byte is dropped, in the same pass, and the caller is
//! told whether a POSIX **quote operator** was seen — a `"`/`'` marker byte or
//! a folded escape pair, since `\` is a quote operator too), [`fold_mask`]
//! reads the quote state from a word's quote mask (the tokenizer strips the
//! quotes from word text).

mod raw;

pub(crate) use raw::fold_quotes;
// `fold` keeps its quote bytes for the delimiter-surfaces pins only.
#[cfg(test)]
pub(crate) use raw::fold;

use crate::mask::{MaskBit, quoted_at};
use alloc::vec::Vec;
use sys::{ShortCStr, ShortCStrError};

/// `bytes` with every unquoted escape pair `\X` folded to `X`, and the quote
/// `mask` aligned to the folded text. The output mask is **derived** from the
/// quoted-ness of each byte, never from its region: a folded word claims no
/// quote-region boundary the mask of the input could not see. The quote state comes from the mask (a
/// word's text has no `"` bytes), so the escaped byte of an unquoted pair
/// comes out protected (`true` — it never splits on IFS or globs), a quoted
/// `\$`/`\\` pair drops its backslash exactly like POSIX #4.2 and like the
/// substitution pass (`substitute/arg.rs`), any other quoted pair keeps it, and
/// a trailing `\` keeps its backslash with its own mask bit.
pub(crate) fn fold_mask(bytes: &[u8], mask: &[MaskBit]) -> (Vec<u8>, Vec<MaskBit>) {
    let mut out = Vec::new();
    let mut out_mask = Vec::new();
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        let quoted = quoted_at(mask, i);
        match bytes.get(i + 1) {
            // Unquoted pair: drop the backslash, protect the escaped byte.
            Some(&c) if b == b'\\' && !quoted => {
                out.push(c);
                out_mask.push(MaskBit::Quoted);
                i += 2;
            }
            // Quoted `\$`, `\\` and `` \` ``: POSIX #4.2 drops the backslash.
            // This is the arm `substitute/arg.rs` runs on a word that *does*
            // go through substitution, so a command name, a redirect target or
            // a builtin name sees the same text either way (task #141's
            // single-quote encoding relies on that).
            Some(&c @ (b'$' | b'\\' | b'`')) if b == b'\\' => {
                out.push(c);
                out_mask.push(MaskBit::from_quoted(quoted));
                i += 2;
            }
            _ => {
                out.push(b);
                out_mask.push(MaskBit::from_quoted(quoted));
                i += 1;
            }
        }
    }
    (out, out_mask)
}

/// [`fold_mask`] on a word, returning the folded word with its aligned mask.
pub(crate) fn fold_word(
    word: &ShortCStr,
    mask: &[MaskBit],
) -> Result<(ShortCStr, Vec<MaskBit>), ShortCStrError> {
    let (bytes, out_mask) = fold_mask(word.as_bytes()?, mask);
    Ok((ShortCStr::from_vec(bytes)?, out_mask))
}

#[cfg(test)]
mod tests;

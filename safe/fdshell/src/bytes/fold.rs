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

use crate::bytes::{QUOTE, SQUOTE};
use alloc::vec::Vec;
use sys::{ShortCStr, ShortCStrError};

/// `raw` with every unquoted escape pair `\X` folded to `X`, quote bytes kept.
/// A `\` inside a quoted span keeps its backslash (POSIX #4.2), a `\` inside a
/// `'…'` span is literal (POSIX §2.2.3 — a single-quoted span has no escape
/// pair at all), and a trailing `\` at the end of `raw` keeps it.
pub(crate) fn fold(raw: &[u8]) -> Vec<u8> {
    fold_impl(raw, true).0
}

/// POSIX quote removal plus escape folding on raw bytes: every `"`/`'`
/// **marker** byte is dropped and every unquoted `\X` pair folds to `X` in the
/// same pass, so a heredoc delimiter word `a'b'c` delimits `abc` and an
/// escaped quote (`a\'b`) survives as data. The flag reports a POSIX
/// **quote operator** (POSIX XCU 2.3: `\` is one), i.e. a marker byte in any
/// position **or a folded unquoted escape pair** — a quoted delimiter makes a
/// here-doc body literal, and `<<E\OF` is quoted exactly like `<<"EOF"`
/// (task #263). An escaped quote is data in the text, and a trailing lone `\`
/// is no pair, so neither alone quotes: `E\` stays unquoted.
pub(crate) fn fold_quotes(raw: &[u8]) -> (Vec<u8>, bool) {
    fold_impl(raw, false)
}

/// The shared state machine. `keep_quotes` writes the quote marker bytes into
/// the output ([`fold`]) or applies quote removal ([`fold_quotes`]); the quote
/// state itself is identical for both, which is why one machine serves both
/// rules. The returned flag is `true` when a POSIX quote operator was seen: a
/// marker byte, or an escape pair the pass folded (`\` is a quote operator,
/// POSIX XCU 2.3). A trailing `\` with no byte to escape is no pair, so it
/// quotes nothing.
fn fold_impl(raw: &[u8], keep_quotes: bool) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut quoted = false;
    let mut single = false;
    let mut has_quote = false;
    while let Some(&b) = raw.get(i) {
        if b == QUOTE && !single {
            quoted = !quoted;
            has_quote = true;
            if keep_quotes {
                out.push(b);
            }
            i += 1;
        } else if b == SQUOTE && !quoted {
            single = !single;
            has_quote = true;
            if keep_quotes {
                out.push(b);
            }
            i += 1;
        } else if b == b'\\' && !quoted && !single {
            i += 1;
            match raw.get(i) {
                // A pair is consumed: POSIX XCU 2.3 counts the `\` a quote
                // operator, so the word that carried it is quoted.
                Some(&c) => {
                    out.push(c);
                    has_quote = true;
                }
                // No pair: the lone `\` keeps its backslash and quotes nothing.
                None => out.push(b),
            }
            i += 1;
        } else {
            out.push(b);
            i += 1;
        }
    }
    (out, has_quote)
}

/// `bytes` with every unquoted escape pair `\X` folded to `X`, and the quote
/// `mask` aligned to the folded text. The quote state comes from the mask (a
/// word's text has no `"` bytes), so the escaped byte of an unquoted pair
/// comes out protected (`true` — it never splits on IFS or globs), a quoted
/// `\$`/`\\` pair drops its backslash exactly like POSIX #4.2 and like the
/// substitution pass (`substitute/arg.rs`), any other quoted pair keeps it, and
/// a trailing `\` keeps its backslash with its own mask bit.
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
            // Quoted `\$`, `\\` and `` \` ``: POSIX #4.2 drops the backslash.
            // This is the arm `substitute/arg.rs` runs on a word that *does*
            // go through substitution, so a command name, a redirect target or
            // a builtin name sees the same text either way (task #141's
            // single-quote encoding relies on that).
            Some(&c @ (b'$' | b'\\' | b'`')) if b == b'\\' => {
                out.push(c);
                out_mask.push(quoted);
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

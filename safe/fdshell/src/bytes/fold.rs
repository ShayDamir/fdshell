//! Escape-pair folding: every unquoted `\X` pair of a byte slice folds to `X`
//! (POSIX #4.1 — the backslash is removed and the escaped byte loses its
//! special meaning), so a word/delimiter text is compared in its folded form.

use crate::bytes::QUOTE;
use alloc::vec::Vec;

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

#[cfg(test)]
mod tests;

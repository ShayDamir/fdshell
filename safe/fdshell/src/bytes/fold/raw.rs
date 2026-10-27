//! Raw-byte folding: the quote state of [`fold`] and [`fold_quotes`] is read
//! off the bytes themselves (see the module doc of `bytes/fold.rs`, which
//! keeps the three-entry-point contract).

use alloc::vec::Vec;

use crate::bytes::{QUOTE, SQUOTE};

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

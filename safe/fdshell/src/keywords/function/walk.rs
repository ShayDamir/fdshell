//! The definer-word walk that mirrors the tokenizer: the word split the byte
//! rule is stated over, and the `$( … )` jump the tokenizer takes (LESSONS
//! "Byte-level definer words must be the tokenizer's tokens", task #277).

use super::leading_ws;
use crate::paren_scan;

/// The first definer word, and the text after it. A `)` is the tokenizer's own
/// separator token, so it is a word of its own: it never joins the word before
/// it, and it is never consumed as a terminator, so a `)` glued after a word is
/// the start of the text returned with it. Leading whitespace is skipped; an
/// empty body gives `None`. A `$( … )` region is one word unit, exactly as the
/// tokenizer reads it ([`word_end`], task #277).
pub(super) fn skip_word(word: &[u8]) -> (Option<&[u8]>, &[u8]) {
    let body = word.get(leading_ws(word)..).unwrap_or(b"");
    let end = word_end(body);
    let word = body
        .get(..end)
        .filter(|w| !w.is_empty())
        .or_else(|| body.get(..1));
    (word, body.get(end..).unwrap_or(b""))
}

/// The region jump of [`word_end`]: the end of the first `$( … )` region, through
/// `paren_scan::scan_at` — the tokenizer's unit, one implementation (task #277).
/// `None` = the word carries no *closed* region: no `$( … )` at all, or one that
/// never closes and is the tokenizer's own report.
pub(super) fn region_end(body: &[u8]) -> Option<usize> {
    let open = body.windows(2).position(|w| w == b"$(")?;
    paren_scan::scan_at(body, open + 2, 1).map(|(_, at)| at)
}

/// The end of the first word of `body`: the walk stops at whitespace or at the
/// tokenizer's `)` separator, and **consumes a `$( … )` region whole** —
/// `parse/token/step.rs` reads `$( … )` as one word, so the byte split must not
/// stop inside a region (LESSONS "Byte-level definer words must be the
/// tokenizer's tokens", task #277). A region carries `(`, `)` and whitespace,
/// so a word that carries one is never a definer word ([`super::name_len`]), in word 0
/// (`$( echo true )`) as much as glued into one (`x=$( echo a )`).
///
/// An unterminated region claims the rest of `body` ([`region_end`] answers
/// `None`, the walk takes the remainder): the tokenizer rejects that
/// input with `unexpected end of input`, so the byte scan must never pair a word
/// behind it with a `{`, and the walk stays terminating.
pub(super) fn word_end(body: &[u8]) -> usize {
    let mut i = 0;
    while let Some(&b) = body.get(i) {
        if let Some(region) = body.get(i..).filter(|r| r.starts_with(b"$(")) {
            i += region_end(region).unwrap_or(region.len());
            continue;
        }
        if b.is_ascii_whitespace() || b == b')' {
            break;
        }
        i += 1;
    }
    i
}

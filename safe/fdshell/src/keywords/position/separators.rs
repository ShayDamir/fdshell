//! The **separator words** of a keyword run: the one set the plain rule of
//! [`super::after`] reads as a command restart, spelled out so the byte-level
//! rows and the token-level gate cannot drift apart (task #336), plus
//! [`word_place`], the place an ordinary word — a quote-protected separator
//! among them — leaves. The parent re-exports all three, so every scorer reads
//! them through it.

// mutants: -C --lib -- -E 'test(/^keywords::position::/)'

use crate::keywords::Place;
use crate::parse::Token;
use crate::parse::word_quoted::word_quoted;

/// The words the plain rule of [`super::after`] reads as a command restart: the
/// separators and the parens, **one set** — the words of
/// [`crate::bytes::word_break`] spelled out — shared with the `for` list's
/// [`super::list::step`] so the two byte-level rows cannot drift, and read back
/// by the token-level gate [`separator_data`] so a token-level walk cannot drift
/// either (task #336). A word that merely *ends* in a word-break byte is not
/// one of these words: the boundary rule (task #396) reads such a word through
/// the keyword rows, and only the glued-`)` clause-close row keeps it.
///
/// The **byte-level** callers of [`super::after`] (`cond/boundary/block`,
/// `comment/depth_delta`) hand it raw line bytes, so a quoted separator's word
/// text there is `";"` / `"|"` — the shape this set does not contain — and they
/// need no gate and no signature change. Only the two token-level callers see a
/// token whose text is the separator, and that is what [`separator_data`]
/// catches.
pub(crate) fn separator_shape(w: &[u8]) -> bool {
    matches!(w, b"" | b";" | b"&&" | b"||" | b"&" | b"(" | b")")
}

/// The place an **ordinary word** leaves: an argument everywhere, except that a
/// `for` word list **stays** a list (task #306's sticky place,
/// [`super::list::step`]). This is what a quote-protected separator leaves —
/// `for i in ";" do …` keeps its list, where a plain `Place::Data` would drop it.
pub(crate) fn word_place(place: Place) -> Place {
    if matches!(place, Place::Words) {
        Place::Words
    } else {
        Place::Data
    }
}

/// `tok` is a **separator-shaped word**: its text is one of [`separator_shape`]'s
/// words **and** its raw span carries quote bytes, so POSIX §2.9.1 makes it word
/// data and not the separator [`super::after`]'s rows read (task #336 — the token-level
/// twin of `parse::word_quoted::span_word`, which is text **and** span, reached
/// through [`word_quoted`]). The text is trimmed as [`super::after`] trims it, because a
/// quoted `" ; "` word carries its spaces in the text.
pub(crate) fn separator_data(tok: &Token) -> bool {
    let (t, start, end, _fq, _mask, _spans) = tok;
    t.as_bytes()
        .is_ok_and(|w| separator_shape(w.trim_ascii()) && word_quoted(t, *start, *end))
}

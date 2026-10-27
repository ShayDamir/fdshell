//! A `case` pattern word with the quote mask parallel to it (task #280).
//!
//! A pattern is a **word**: it is folded and expanded with its own mask before
//! the matcher reads it, so `match_component` sees a masked `\` as a literal
//! backslash and the single-quote encoding (`'a$b'` reaches the text as
//! `a\$b`) folds away.

use crate::mask::{MaskBit, quoted_at};
use crate::parse::Token;
use crate::parse::semi::try_join_masked;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

/// One `|`-separated `case` pattern: the joined word text and its per-byte
/// quote mask.
pub struct Pattern {
    /// The joined pattern word, quote bytes stripped, escape pairs intact.
    pub text: ShortCStr,
    /// Per-byte quote mask, parallel to `text` (`false` for the space
    /// `try_join_masked` inserts between two tokens, so a joined space is
    /// never a pattern byte).
    pub mask: Vec<MaskBit>,
    /// Zero-byte quote-span indices, parallel to `mask` (task #321).
    pub spans: Vec<usize>,
}

/// Join one pattern's tokens into a [`Pattern`]. `trim_semi` stays with the
/// caller: `try_join_masked` is the single owner of the mask's offsets.
pub(crate) fn join(tokens: &[Token]) -> Pattern {
    let (text, mask, spans) = try_join_masked(tokens);
    Pattern { text, mask, spans }
}

/// The token that opens a clause's pattern list, with bash's optional leading
/// `(` taken off (POSIX 2.9.4.3 spells a clause `( pattern-list )`, bash 5.3.9
/// strips the `(`). `None`: the token carries no such `(`, so the clause walk
/// starts where it was. `Some(tokens)`: the byte was taken, and `tokens` is what
/// is left of the token — **empty** when the token *is* the `(` (POSIX's spaced
/// `( pattern )`), so `case x in ()` keeps fdshell's empty-pattern verdict and
/// [`try_join_masked`] gains no leading space.
///
/// The byte is taken only when that byte's mask bit is `Unquoted` **and** the
/// text starts with `(` — the same two signals `stray::opens_groups` reads for a
/// paren: a quoted `"(a*` / `'('a*` keeps the bit (measured: no strip, bash
/// agrees), and an escaped `\(a*` keeps the whole pair in the text, so the paren
/// is not the leading byte. A `(` behind a `|` is never reached: the caller asks
/// about the clause's **first** token only, and one byte is taken at that (the
/// `((a*))` shape keeps its inner `(`).
///
/// Text, mask, spans and the raw span move together, so the slice stays
/// self-consistent for [`try_join_masked`] (the mask's only owner) and for
/// `word_quoted`'s `end - start` test: byte 0 goes, bit 0 goes, every span index
/// steps back one (a span at 0 is a quote pair written in front of the `(` and
/// stays at 0), and `start` follows the text.
pub(crate) fn take_open_paren(token: &Token) -> Option<Vec<Token>> {
    let (text, start, end, fully_quoted, mask, spans) = token;
    // The mask is the discriminator, never the per-word `fully_quoted` flag:
    // only an unquoted leading `(` is the token POSIX writes, the rest is text.
    if quoted_at(mask, 0) {
        return None;
    }
    // The byte itself: an escaped `\(a*` carries the pair's `\` at index 0, so
    // there is no leading paren to take.
    let stripped = text.strip_prefix(b"(")?;
    // The token *is* the `(` (POSIX's spaced `( pattern )`): nothing is left of
    // it, so the caller's pattern list starts empty.
    if stripped.is_empty() {
        return Some(Vec::new());
    }
    let opened: Token = (
        stripped,
        start + 1,
        *end,
        *fully_quoted,
        mask.iter().skip(1).copied().collect(),
        spans.iter().map(|&i| i.saturating_sub(1)).collect(),
    );
    Some(vec![opened])
}

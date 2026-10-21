//! A `case` pattern word with the quote mask parallel to it (task #280).
//!
//! A pattern is a **word**: it is folded and expanded with its own mask before
//! the matcher reads it, so `match_component` sees a masked `\` as a literal
//! backslash and the single-quote encoding (`'a$b'` reaches the text as
//! `a\$b`) folds away.

use crate::mask::MaskBit;
use crate::parse::Token;
use crate::parse::semi::try_join_masked;
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

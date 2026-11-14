//! The parsed `for` block: the data shape its builders produce and the run
//! layer consumes.

use crate::mask::MaskBit;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use sys::{ScriptText, ShortCStr};

#[cfg_attr(test, derive(Debug))]
pub struct ForBlock {
    pub var: ShortCStr,
    pub words: Vec<ShortCStr>,
    /// Per-byte quote masks, parallel to `words`.
    pub words_mask: Vec<Vec<MaskBit>>,
    /// Zero-byte quote-span indices per word (task #321), parallel to
    /// `words_mask`.
    pub words_span: Vec<Vec<usize>>,
    /// Whether each word's raw span contained quotes, parallel to `words`
    /// (see `parse::word_quoted`).
    pub words_quoted: Vec<bool>,
    pub body: ScriptText,
    /// The redirections written after the closing `done` (task #298,
    /// `parse::block_tail`); they apply to the whole loop and open **before**
    /// the word list is expanded (bash's order, measured).
    pub redirects: Vec<RedirectDef>,
    /// The clause had **no** `in` (task #416), so its word list is the
    /// positional parameters at run time — `"$@"`-shaped: one word per
    /// parameter, verbatim, nothing when none are set. `words` is empty here;
    /// the empty list of `for name in; do` means zero iterations, and the flag
    /// is the whole distinction between the two rows.
    pub words_default_to_positional: bool,
}

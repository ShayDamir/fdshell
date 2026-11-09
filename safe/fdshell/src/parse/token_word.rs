//! The parser's word token as a named-field struct — the six slots of the
//! former `parse.rs` tuple alias, each named (task #549; the method migration
//! is the next child of epic #547, so no methods live here yet).

use crate::mask::MaskBit;
use alloc::vec::Vec;
use sys::ShortCStr;

/// A token: its word, the raw byte range it was cut from, and the quote
/// bookkeeping that survived quote-stripping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// The quote-stripped word (empty for a quoted empty word such as `""`).
    pub(crate) word: ShortCStr,
    /// Start of the byte range of the token's raw text.
    pub(crate) start: usize,
    /// End (exclusive) of that same raw-text byte range.
    pub(crate) end: usize,
    /// Whether the word was fully quoted.
    pub(crate) fully_quoted: bool,
    /// The per-byte quote mask, parallel to the word; `true` marks bytes that
    /// were inside quotes (double **or** single) and are protected from IFS
    /// word splitting.
    pub(crate) mask: Vec<MaskBit>,
    /// The word-text indices at which a quote span closed **without
    /// contributing any byte** (task #321: the mask cannot see a `""` / `''`
    /// pair — zero bytes, zero bits — so the tilde rule consults this list;
    /// `~""/x` → text `~/x`, mask all-`false`, spans `[1]`). Parallel
    /// plumbing: every surface that carries a mask carries this list at the
    /// same offset.
    pub(crate) spans: Vec<usize>,
}

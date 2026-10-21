//! The per-byte quote mask of a word.
//!
//! A word carries one [`MaskBit`] per word byte, parallel to its text. It says
//! more than "quoted or not": it spells out **where a quote region starts**. A
//! `bool` cannot say that, because the tokenizer strips the quote bytes — the
//! text of `"$v""b"` is `$vb`, and nothing in it tells where the first quoted
//! region ended, so a bare name keeps collecting across the boundary (bash ends
//! it at the closing quote of the region holding the `$`).
//!
//! Only the tokenizer emits `QuotedStart` (`parse/token/mask.rs`). A mask
//! derived from a plain `bool` — expansion output, a folded word, a synthesized
//! operator word — goes through [`MaskBit::from_quoted`] and never claims a
//! boundary it cannot see. A mask slice keeps its bits verbatim, so a slice
//! that starts mid-region starts with `Quoted`, which is exactly right.

/// The quote state of one word byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MaskBit {
    /// Outside quotes.
    Unquoted,
    /// The first byte of a quote region: the byte an opening `"`/`'` (or the
    /// byte a previous region closed in front of).
    QuotedStart,
    /// A byte continuing the region opened by a `QuotedStart`.
    Quoted,
}

impl MaskBit {
    /// The `bool` reading of the old mask: is this byte inside quotes?
    pub(crate) const fn quoted(self) -> bool {
        !matches!(self, Self::Unquoted)
    }

    /// The bit for a byte known only as quoted/not-quoted. It never marks a
    /// region start, so a derived mask cannot invent a region boundary.
    pub(crate) const fn from_quoted(quoted: bool) -> Self {
        if quoted { Self::Quoted } else { Self::Unquoted }
    }
}

/// Is byte `i` of `mask` quoted? A bit past the end of the mask is unquoted:
/// the padding rule every mask-independent caller relies on (a heredoc body,
/// `case_exec`'s joined word, the `~`-prefix scan).
pub(crate) fn quoted_at(mask: &[MaskBit], i: usize) -> bool {
    mask.get(i).copied().is_some_and(MaskBit::quoted)
}

/// Bytes a name arm may consume from `from`: the length of the quote region
/// that byte sits in — the run of `Quoted` after a `QuotedStart`, the run of
/// `Unquoted` after an unquoted byte. A name cannot cross out of the region its
/// trigger byte sits in, because the tokenizer's quote regions are what end a
/// name: bash expands `"$v"bc` as the parameter `v` plus the literal `bc`, and
/// `"$v""b"` as `v` plus `b` — the second `"` opens a **new** region, and its
/// `QuotedStart` stops the run. A missing bit (a mask shorter than the word
/// text) carries no information, so the run is unbounded and the collection is
/// mask-independent.
pub(crate) fn region_len(mask: &[MaskBit], from: usize) -> usize {
    let Some(&bit) = mask.get(from) else {
        return usize::MAX;
    };
    // The byte that keeps the region open: any `Quoted`, or `Unquoted` itself.
    let next = MaskBit::from_quoted(bit.quoted());
    1 + mask
        .iter()
        .skip(from + 1)
        .take_while(|&b| *b == next)
        .count()
}

#[cfg(test)]
pub(crate) mod tests;

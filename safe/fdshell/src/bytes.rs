//! Shared ASCII byte constants for the byte-level scanners.

pub(crate) mod fold;

/// The `"` byte, shared by the quote-state scanners (see LESSONS.md on why
/// this is a constant rather than a `b'"'` literal at each use site).
pub(crate) const QUOTE: u8 = b'"';

/// The `'` byte, shared by the single-quote-state scanners. A named constant
/// for the same tokei reason as [`QUOTE`].
pub(crate) const SQUOTE: u8 = b'\'';

/// The byte that ends a bash word: whitespace and the metacharacters `;`,
/// `|`, `&`, `<`, `>`, `(`, `)` — the one definition of the reserved-word
/// boundary (task #396). A **keyword word** ends at it at both
/// granularities: the byte level through [`crate::keywords::quoted::quoted_word`]
/// (the scan-back stops at it) and the recognition boundary
/// ([`crate::keywords::kind::boundary`], measured by
/// [`crate::segment::block::length::opener_len`]); the tokenizer ends a word
/// at it when the bytes so far are exactly a reserved word
/// ([`crate::keywords::reserved`], `parse::token::step::keyword`). A
/// non-keyword word keeps fdshell's current segmentation — bash's general
/// break is the open work of tasks #316/#470/#422/#499.
pub(crate) fn word_break(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')')
}

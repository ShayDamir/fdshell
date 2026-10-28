//! The `for` block's word list: the `in` words with their quote masks and
//! per-word quoted flags.

use super::super::Token;
use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::parse::semi::trim_semi;
use crate::parse::word_quoted::{self, span_word};
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

/// The `in` words of a `for` block, with per-byte quote masks (parallel to
/// the words) and per-word quoted flags (see `parse::word_quoted`).
pub(super) struct ForWords {
    pub(super) words: Vec<ShortCStr>,
    pub(super) words_mask: Vec<Vec<MaskBit>>,
    /// Zero-byte quote-span indices per word (task #321), parallel to the
    /// words and their masks.
    pub(super) words_span: Vec<Vec<usize>>,
    pub(super) words_quoted: Vec<bool>,
}

/// The plain `&` word of a `for` word list: bash's list ends at the async
/// operator and its parser fails the `do` expectation there (task #360), so a
/// list that carries one is not a word list. The gate is [`span_word`]'s raw
/// span, so a quoted `"&"` / `'&'`, an escaped `\&` (the tokenizer keeps the
/// pair in the word text) and a glued `x&` / `&&` are word **data** — measured:
/// bash prints `&` for the quoted and escaped forms, and the glued rows are the
/// accepted divergences of task #496.
///
/// Parse-time rule: a `&` that *expansion* produces (`for i in $(echo '&')`)
/// stays a word, as in bash.
pub(super) fn plain_amp(tokens: &[Token]) -> bool {
    tokens.iter().any(|tok| span_word(tok, b"&"))
}

/// The word list of a `for` block (the `in` words up to `do`).
pub(super) fn collect_words(
    tokens: &[Token],
    in_pos: usize,
    do_idx: usize,
) -> Result<ForWords, Report<ParseError>> {
    let word_tokens = trim_semi(
        tokens
            .get(in_pos + 1..do_idx)
            .ok_or(ParseError::ExpectedWordList)?,
    );
    let words: Vec<ShortCStr> = word_tokens
        .iter()
        .map(|(t, _, _, _, _, _)| t.clone())
        .collect();
    let words_mask: Vec<Vec<MaskBit>> = word_tokens
        .iter()
        .map(|(_, _, _, _, m, _)| m.clone())
        .collect();
    let words_span: Vec<Vec<usize>> = word_tokens
        .iter()
        .map(|(_, _, _, _, _, s)| s.clone())
        .collect();
    let words_quoted: Vec<bool> = word_tokens
        .iter()
        .map(|(t, s, e, _, _, _)| word_quoted::word_quoted(t, *s, *e))
        .collect();
    Ok(ForWords {
        words,
        words_mask,
        words_span,
        words_quoted,
    })
}

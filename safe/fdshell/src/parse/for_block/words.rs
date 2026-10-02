//! The `for` block's word list: the `in` words with their quote masks and
//! per-word quoted flags.

use super::super::Token;
use crate::error::parse::ParseError;
use crate::parse::semi::trim_semi;
use crate::parse::word_quoted;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

/// The `in` words of a `for` block, with per-byte quote masks (parallel to
/// the words) and per-word quoted flags (see `parse::word_quoted`).
pub(super) struct ForWords {
    pub(super) words: Vec<ShortCStr>,
    pub(super) words_mask: Vec<Vec<bool>>,
    pub(super) words_quoted: Vec<bool>,
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
    let words: Vec<ShortCStr> = word_tokens.iter().map(|(t, _, _, _, _)| t.clone()).collect();
    let words_mask: Vec<Vec<bool>> = word_tokens.iter().map(|(_, _, _, _, m)| m.clone()).collect();
    let words_quoted: Vec<bool> =
        word_tokens.iter().map(|(t, s, e, _, _)| word_quoted::word_quoted(t, *s, *e)).collect();
    Ok(ForWords {
        words,
        words_mask,
        words_quoted,
    })
}

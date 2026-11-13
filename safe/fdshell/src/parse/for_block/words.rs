// mutants: -C --lib -- -E 'test(/^parse::for_block::/)'

//! The `for` block's word list: the `in` words with their quote masks and
//! per-word quoted flags, and the ONE operator-word rule that decides which of
//! them are words at all (task #524).

use super::super::Token;
use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::parse::semi::{gap_run_at_end, trim_semi};
use crate::parse::word_quoted::slot_rejects;
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

/// Whether a `for` word list carries an operator word it does not own: bash's word
/// list accepts **words only**, every operator token ends it, and its parser fails
/// the `do` expectation there (task #360 lands the plain `&`, task #496 the rest,
/// task **#470** the parens). The gate is the shared word-slot rule
/// [`slot_rejects`] with an **owns-nothing** closure: this position owns no row of
/// the ONE set (the `;`/newline that terminates the list is excluded by the slice
/// below, not by the set), so a word-opening `(` and the bare `)` reject like the
/// `&&`/`|` family always did. A quoted `"&"` / `'&'` / `"&&"` / `"(a)"`, an
/// escaped `\&` / `\(` (the tokenizer keeps the pair in the word text) and a glued
/// `x&` / `x&&` / `a(b` are word **data** — measured: bash prints the quoted and
/// escaped forms at rc 0, and the glued rows stay (the surviving divergence).
///
/// Parse-time rule: a `&` that *expansion* produces (`for i in $(echo '&')`)
/// stays a word, as in bash. The `owned` data stays load-bearing through the
/// `case` word slot's `;` (task #470; `word_quoted/tests.rs` pins it).
///
/// The exclusion is **asymmetric** (task #559): the list's own terminator is
/// bash's trailing **gap run** — at most one real `;` byte, then any run of
/// newlines, read from the end through [`gap_run_at_end`] — while a `;`
/// anywhere else, the leading side included, is a real operator word
/// (`for i in ; 1; do` is bash's rc 2).
pub(super) fn rejects_operator_word(tokens: &[Token], line: &[u8]) -> bool {
    // The trailing gap run is the list's own terminator — the tokens the `do`
    // is preceded by — so the list proper is everything before it.
    let gap = gap_run_at_end(tokens, line);
    let list = tokens.get(..tokens.len() - gap).unwrap_or(&[]);
    let owns_nothing = |_w: &[u8]| false;
    list.iter()
        .any(|tok| slot_rejects(tok, line, &owns_nothing))
}

/// The word list of a `for` block (the `in` words up to `do`).
///
/// `anchor` is the token the list starts behind: the `in`, or — for a clause
/// with no `in` (task #416) — the name, so the slot holds only the terminator
/// and the list comes back empty.
pub(super) fn collect_words(
    tokens: &[Token],
    anchor: usize,
    do_idx: usize,
) -> Result<ForWords, Report<ParseError>> {
    let word_tokens = trim_semi(
        tokens
            .get(anchor + 1..do_idx)
            .ok_or(ParseError::ExpectedWordList)?,
    );
    let words: Vec<ShortCStr> = word_tokens
        .iter()
        .map(|Token { word: t, .. }| t.clone())
        .collect();
    let words_mask: Vec<Vec<MaskBit>> = word_tokens
        .iter()
        .map(|Token { mask: m, .. }| m.clone())
        .collect();
    let words_span: Vec<Vec<usize>> = word_tokens
        .iter()
        .map(|Token { spans: s, .. }| s.clone())
        .collect();
    let words_quoted: Vec<bool> = word_tokens
        .iter()
        .map(
            |Token {
                 word: t,
                 start: s,
                 end: e,
                 ..
             }| t.is_shorter_than_span(*s, *e),
        )
        .collect();
    Ok(ForWords {
        words,
        words_mask,
        words_span,
        words_quoted,
    })
}

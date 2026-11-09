// mutants: -C --lib -- -E 'test(/^parse::for_block::/)'

//! The `for` block's word list: the `in` words with their quote masks and
//! per-word quoted flags, and the ONE operator-word rule that decides which of
//! them are words at all (task #524).

use super::super::Token;
use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::parse::semi::trim_semi;
use crate::parse::word_quoted::operator_word;
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

/// The words the `for` word list **owns** as its own separator: the parens are
/// task **#470**'s (an operator word in a word slot is that task's decision), and
/// the `;`/newline that terminates the list before `do` is the list's own, so the
/// caller's slice ends with it and it is never rejected. Everything else in the ONE
/// set that lands in a word slot is the operator word bash ends its word list at —
/// which is exactly what tasks #495/#496 measured.
fn owns(w: &[u8]) -> bool {
    matches!(w, b"(" | b")")
}

/// Whether a `for` word list carries an operator word it does not own: bash's word
/// list accepts **words only**, every operator token ends it, and its parser fails
/// the `do` expectation there (task #360 lands the plain `&`, task #496 the rest).
/// The gate is the ONE predicate [`operator_word`], so a quoted `"&"` / `'&'` /
/// `"&&"`, an escaped `\&` (the tokenizer keeps the pair in the word text) and a
/// glued `x&` / `x&&` are word **data** — measured: bash prints `&` for the quoted
/// and escaped forms, and the glued rows stay.
///
/// Parse-time rule: a `&` that *expansion* produces (`for i in $(echo '&')`)
/// stays a word, as in bash.
pub(super) fn rejects_operator_word(tokens: &[Token], line: &[u8]) -> bool {
    // The trailing `;`/newline is the list's own terminator — the token the `do`
    // is preceded by — so the list proper is everything before it.
    let list = tokens.get(..tokens.len().saturating_sub(1)).unwrap_or(&[]);
    list.iter()
        .any(|tok| operator_word(tok, line, &owns).is_some())
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

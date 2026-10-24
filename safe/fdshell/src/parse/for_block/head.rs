//! The **head** of a `for` block (task #439): the `for` keyword, its variable
//! name, and the `in` that opens the word list. The head is the whole
//! `for … in` prefix, and its three errors are the parse contract of that
//! prefix (`ExpectedFor`, `ExpectedVariableName`, `ExpectedIn`, STYLE.md §4);
//! the word list, the body and the closer walk stay with
//! [`super::tokens_to_for`], which is the only caller.

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::word_quoted::keyword_word;
use error_stack::{Report, ensure};
use sys::ShortCStr;

/// The `for` head: check the `for` keyword, take the variable name, and find
/// the `in` that opens the word list.
///
/// Returns the variable word and the index of the `in` token — the index the
/// caller measures its `do` search and its word list from. `line` is the block
/// text the tokens index into, so `keyword_word` can tell a real `in` from a
/// quoted word (`for x in "in"`, task #293).
pub(super) fn head(
    tokens: &[Token],
    line: &[u8],
) -> Result<(ShortCStr, usize), Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"for")),
        ParseError::ExpectedFor
    );
    let var = tokens
        .get(1)
        .ok_or(ParseError::ExpectedVariableName)?
        .0
        .clone();
    let in_pos = tokens
        .iter()
        .skip(2)
        .position(|tok| keyword_word(tok, line, b"in"))
        .ok_or(ParseError::ExpectedIn)?
        + 2;
    Ok((var, in_pos))
}

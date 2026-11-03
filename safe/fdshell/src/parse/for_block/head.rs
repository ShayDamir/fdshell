//! The **head** of a `for` block (task #439): the `for` keyword, its variable
//! name, and the `in` that opens the word list. POSIX #2.9.1 makes that `in`
//! **optional** (task #416): a clause without it is `for name`, and its list is
//! the positional parameters. The head is the whole `for [name [in]]` prefix,
//! and its three errors are the parse contract of that prefix (`ExpectedFor`,
//! `ExpectedVariableName`, `ExpectedIn`, STYLE.md §4 — `ExpectedIn` is now the
//! error of a word sitting where the optional `in` would be); the word list, the
//! body and the closer walk stay with [`super::tokens_to_for`], the only caller.

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::word_quoted::{keyword_word, span_word};
use error_stack::{Report, ensure};
use sys::ShortCStr;

/// The `for` head: check the `for` keyword, take the variable name, and find
/// the `in` that opens the word list.
///
/// Returns the variable word and the index of the `in` token — the index the
/// caller measures its `do` search and its word list from — or `None` when the
/// clause has no `in` (task #416): POSIX makes the `in compound-list` optional,
/// and bash then reads `"$@"`, so the caller takes the list from the positional
/// parameters. `line` is the block text the tokens index into, so `keyword_word`
/// can tell a real `in` from a quoted word (`for x in "in"`, task #293).
pub(super) fn head(
    tokens: &[Token],
    line: &[u8],
) -> Result<(ShortCStr, Option<usize>), Report<ParseError>> {
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
    let Some(in_pos) = tokens
        .iter()
        .skip(2)
        .position(|tok| keyword_word(tok, line, b"in"))
    else {
        // The optional-`in` clause is `for name` and nothing else: the slot
        // behind the name holds only the clause terminator — the same `span_word`
        // test `semi/preceded.rs`'s `Term::Semi` arm makes (task #247). Any word
        // there is `ExpectedIn` (`for i 1 2`, `for i "in"`, bash rc 2).
        ensure!(
            tokens.get(2).is_some_and(|tok| span_word(tok, b";")),
            ParseError::ExpectedIn
        );
        return Ok((var, None));
    };
    Ok((var, Some(in_pos + 2)))
}

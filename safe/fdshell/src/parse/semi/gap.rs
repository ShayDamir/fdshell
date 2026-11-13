// mutants: -C --lib -- -E 'test(/^parse::semi::/)'

//! The byte test that separates bash's two separator spellings, and bash's
//! terminator **gap** — at most one real `;` byte, then any run of newlines —
//! read from either end of a token slice. The tokenizer folds a `;` byte, a
//! newline and `)` into one `;` token ([`crate::parse::token`]), so only the
//! raw byte tells them apart, and only the **test** has one owner here with the
//! byte as data (task #559). The two directions are one rule, not mirrors:
//! [`gap_run`] reads the run at the **start** of the slice (the arithmetic
//! `for` tail — tasks #400/#394), [`gap_run_at_end`] the same run at the **end**
//! (the word-form `for` list — task #559). A `;` **behind** a newline, a second
//! `;` (`; ;`, `;;`) and a `&` stay outside the run: bash's rc-2 family.
//! Comment rows need no arm in either direction — the tokenizer drops them
//! (with the newline that ends the row) before any of this runs, so what
//! reaches a gap is only the newline that ended the row **before** a comment.

use super::Token;

/// The `;` **byte** behind a separator token — the byte test with one owner,
/// the byte as data ([`semi_byte`], [`gap_run`], [`gap_run_at_end`]). Both
/// words go through [`Token::span_word`], so a quoted `";"` word (and an
/// escaped `\;`, whose pair the tokenizer keeps in the word text) is word data
/// and never the byte (tasks #336/#396). `false` when the byte at the token's
/// start is not `byte`.
///
/// Stays free: it reads the raw script text `line` for the byte behind the `;`
/// token, so its data is not the token's own state (task #550).
pub(crate) fn separator_byte(tok: &Token, line: &[u8], byte: u8) -> bool {
    let Token { start, .. } = tok;
    tok.span_word(b";") && line.get(*start) == Some(&byte)
}

/// The `;`-byte reading of [`separator_byte`] — kept as the name its three
/// readers call (task #355: `do echo a & ; done` is a syntax error while
/// `do echo a &\ndone` is valid; POSIX #2.9.1's `newline_list` slot of the
/// `case` clause list, task #415, is the same split).
pub(crate) fn semi_byte(tok: &Token, line: &[u8]) -> bool {
    separator_byte(tok, line, b';')
}

/// The gap **at the start** of the slice: at most one real `;` byte, then any
/// run of newlines — bash gives the tail one `simple_list_terminator` slot and
/// absorbs the newlines **behind** it (`)) ;\ndo`, `))\n\ndo`), so a `;`
/// behind a newline and a second `;` stay parse errors (bash's rc-2 pair,
/// measured; the pinned twin of the arithmetic `for` is
/// `parse/for_block/tests.rs::a_run_of_terminators_before_the_body_keyword_is_accepted`).
pub(crate) fn gap_run(tokens: &[Token], line: &[u8]) -> usize {
    let mut run = 0;
    if tokens
        .get(run)
        .is_some_and(|t| separator_byte(t, line, b';'))
    {
        run += 1;
    }
    while tokens
        .get(run)
        .is_some_and(|t| separator_byte(t, line, b'\n'))
    {
        run += 1;
    }
    run
}

/// The same run at the **end** of the slice — bash's terminator slot of the
/// word-form `for` word list, which precedes the `do` (task #559: bash runs
/// `for i in 1 2 ;` + newline run + `do`; the leading side is **not** the gap,
/// `for i in ; 1; do` stays bash's rc 2, so the trim stays asymmetric).
pub(crate) fn gap_run_at_end(tokens: &[Token], line: &[u8]) -> usize {
    let mut run = 0;
    while back_byte(tokens, line, run, b'\n') {
        run += 1;
    }
    if back_byte(tokens, line, run, b';') {
        run += 1;
    }
    run
}

/// [`separator_byte`] at `run` tokens **back from the end** of `slice` (`run 0`
/// is its last token; `false` past the front).
fn back_byte(tokens: &[Token], line: &[u8], run: usize, byte: u8) -> bool {
    let Some(idx) = tokens.len().checked_sub(run + 1) else {
        return false;
    };
    tokens
        .get(idx)
        .is_some_and(|tok| separator_byte(tok, line, byte))
}

#[cfg(test)]
mod tests;

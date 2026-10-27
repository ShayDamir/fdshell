//! The clause-keyword search of `parse/semi.rs`: **what may precede** a block
//! clause keyword. POSIX #2.9.1 reads a compound-list terminator before
//! `then`/`elif`/`else` and the `while`/`until` `do`, and the plain `&` is one
//! of them (task #354: `if true & then :; fi` is valid), while a `for` word
//! list takes no list terminator at all (`for i in 1 & do` is bash's syntax
//! error) — hence [`Term`]. The same "what may precede" question is asked about
//! a block's **closing keyword** by [`is_closer_predecessor`] /
//! [`ensure_before_closer`] (task #301), whose answer adds the closer of a
//! compound command to the terminator set.

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::word_quoted::{keyword_word, span_word};
use error_stack::{Report, ensure};

/// The set of words that may precede a block clause keyword.
#[derive(Clone, Copy)]
pub(crate) enum Term {
    /// `;` only: the `for` word list, whose `do` follows the list.
    Semi,
    /// `;` and the plain `&`: a compound list (`if`/`while`/`until` clause
    /// keywords, task #354).
    SemiAmp,
}

/// A command-list terminator: POSIX's `;` (a newline is tokenized as one) or
/// the plain `&` that ends the statement it backgrounds (task #354). Both
/// through [`span_word`], so a quoted `";"` / `"&"` word terminates nothing
/// (task #336) — the rule `group_block.rs` (the `;` before `}`) and
/// [`is_closer_predecessor`] read.
pub(crate) fn is_list_terminator(tok: &Token) -> bool {
    span_word(tok, b";") || span_word(tok, b"&")
}

/// The closing keyword of every block form, as a possible **predecessor** of
/// another block's closer (see [`is_closer_predecessor`]).
const CLOSERS: [&[u8]; 5] = [b"fi", b"done", b"esac", b"}", b")"];

/// The token that may stand in front of a block's closing keyword: POSIX #2.9's
/// command-list terminator ([`is_list_terminator`] — the `;` a newline tokenizes
/// as, and the plain `&`), **or** the closer of the compound command that is the
/// body's last statement: bash's grammar makes the `;` after a compound command
/// optional, so `while :; do if :; then :; fi done` and
/// `if true; then { echo a; } fi` run (rc 0, measured; the
/// [`after`](crate::keywords::after) state machine's closer arm is the scan-side
/// twin, task #301).
///
/// The keyword test is [`keyword_word`]'s raw-span gate (task #247): a quoted
/// `"fi"` word is not a predecessor, and neither is a closer glued to a
/// redirection (`}>f`, `esac>f`) — bash rejects
/// `while false; do { :; }>f done` rc 2 (measured) and this keeps it a parse
/// error. `false` when `idx` is off the list.
pub(crate) fn is_closer_predecessor(tokens: &[Token], line: &[u8], idx: usize) -> bool {
    tokens.get(idx).is_some_and(|tok| {
        is_list_terminator(tok) || CLOSERS.iter().any(|kw| keyword_word(tok, line, kw))
    })
}

/// The `;`-before-closer rule at one block builder: `err` when the closing
/// keyword at `closer_idx` — an index the closing-keyword walk hands back, so
/// never `0` — has no token [`is_closer_predecessor`] accepts in front of it.
/// `for` pinned the rule first (task #233); task #301 is the rule for
/// `if`/`while`/`until` too, with the predecessor set bash accepts.
pub(crate) fn ensure_before_closer(
    tokens: &[Token],
    line: &[u8],
    closer_idx: usize,
    err: ParseError,
) -> Result<(), Report<ParseError>> {
    ensure!(is_closer_predecessor(tokens, line, closer_idx - 1), err);
    Ok(())
}

/// The `;` **token** at `idx` of `tokens` is glued right behind a plain `&`
/// when `tokens[idx-1]` is the `&` and the byte the token points at in `line`
/// is a real `;`. The byte test is the whole rule: the tokenizer emits a `;`
/// token for a `;` byte, a newline and `)` alike ([`crate::parse::token`]),
/// and bash's semantics distinguish them — `do echo a & ; done` is a syntax
/// error, `do echo a &\ndone` is valid (task #355). A newline token is never
/// glue; `false` at index 0, on any word behind a non-`&`, and off the end.
/// Both words of the pair go through [`span_word`], so a quoted `";"` glued
/// behind a quoted `"&"` is word data and no glue (task #336).
pub(crate) fn amp_semi(tokens: &[Token], line: &[u8], idx: usize) -> bool {
    let Some(tok @ (_t, start, _, _, _, _)) = tokens.get(idx) else {
        return false;
    };
    if !span_word(tok, b";") || line.get(*start) != Some(&b';') {
        return false;
    }
    let Some(prev) = idx.checked_sub(1) else {
        return false;
    };
    tokens.get(prev).is_some_and(|p| span_word(p, b"&"))
}

/// The terminator set of one [`Term`] test.
fn precedes(tok: &Token, term: Term) -> bool {
    match term {
        Term::Semi => span_word(tok, b";"),
        Term::SemiAmp => is_list_terminator(tok),
    }
}

/// The keyword word `needle` preceded by a terminator of `term`, at or after
/// `start`: `needle` is a keyword only when it carries no quote byte
/// ([`keyword_word`], task #247), so a quoted `then`/`do`/`else` is a command
/// word and the block reports its own missing keyword (bash: a syntax error).
pub(crate) fn find_preceded_by(
    tokens: &[Token],
    start: usize,
    needle: &[u8],
    line: &[u8],
    term: Term,
) -> Option<usize> {
    for (i, tok) in tokens.iter().enumerate().skip(start) {
        let preceded = i > 0 && tokens.get(i - 1).is_some_and(|tok| precedes(tok, term));
        if keyword_word(tok, line, needle) && preceded {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests;

//! The clause-keyword search of `parse/semi.rs`: **what may precede** a block
//! clause keyword. POSIX #2.9.1 reads a compound-list terminator before
//! `then`/`elif`/`else` and the `while`/`until` `do`, and the plain `&` is one
//! of them (task #354: `if true & then :; fi` is valid), while a `for` word
//! list takes no list terminator at all (`for i in 1 & do` is bash's syntax
//! error) — hence [`Term`].

use super::Token;
use crate::parse::word_quoted::keyword_word;

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
/// the plain `&` that ends the statement it backgrounds (task #354).
pub(crate) fn is_list_terminator(tok: &Token) -> bool {
    tok.0.eq_bytes(b";") || tok.0.eq_bytes(b"&")
}

/// The `;` **token** at `idx` of `tokens` is glued right behind a plain `&`
/// when `tokens[idx-1]` is the `&` and the byte the token points at in `line`
/// is a real `;`. The byte test is the whole rule: the tokenizer emits a `;`
/// token for a `;` byte, a newline and `)` alike ([`crate::parse::token`]),
/// and bash's semantics distinguish them — `do echo a & ; done` is a syntax
/// error, `do echo a &\ndone` is valid (task #355). A newline token is never
/// glue; `false` at index 0, on any word behind a non-`&`, and off the end.
pub(crate) fn amp_semi(tokens: &[Token], line: &[u8], idx: usize) -> bool {
    let Some((t, start, _, _, _, _)) = tokens.get(idx) else {
        return false;
    };
    if !t.eq_bytes(b";") || line.get(*start) != Some(&b';') {
        return false;
    }
    tokens
        .get(idx.saturating_sub(1))
        .is_some_and(|(p, _, _, _, _, _)| idx > 0 && p.eq_bytes(b"&"))
}

/// The terminator set of one [`Term`] test.
fn precedes(tok: &Token, term: Term) -> bool {
    match term {
        Term::Semi => tok.0.eq_bytes(b";"),
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

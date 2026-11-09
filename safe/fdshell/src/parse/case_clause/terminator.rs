// mutants: -C --lib -C --test -C case -- -E 'test(/^parse::case_clause::/) | binary(case)'

//! The clause terminators of the `case` walk (task #420): `;;` stops, `;&` runs
//! the next clause, `;;&` tests the next clause's patterns against the same word.
//! The scan of `extract.rs` and the kind a clause carries are the two halves of
//! one rule and live here together, so they cannot drift (STYLE 2.1/2.6, LESSONS
//! "one predicate").
//!
//! `;&` and `;;&` reach the parser as **two and three tokens**: the tokenizer emits
//! a token only for `;`, a newline and `)` (`parse/token/step.rs`,
//! `token/step/sep.rs`), and `&` is an ordinary word byte — so **the raw span is
//! what distinguishes `;&` from `; &`**: only a *glued* pair is the terminator
//! token, and the new arms are glued-only. The `;;` row keeps its existing
//! semantics, non-glued spellings included (decision D2): that is what holds the
//! `&`-arm family of tasks #354/#355 (`& ;;`, `&;;`, `& ; ;;`, `; &`) exactly as
//! measured.
//!
//! A terminator's **head is a real `;` byte** ([`semi_byte`]'s test, the one task
//! #355/#415 own for the `& ;` split): the tokenizer's `;` token doubles as a
//! newline, and a newline written in front of a glued `;&` is bash's *list*
//! separator, not the terminator's head — `echo A\n;& b)` is the `;&` row bash
//! accepts, and its price is the `;;\n&` row, which stays divergent (decision D1).

use super::Token;
use crate::parse::semi::semi_byte;
use crate::parse::word_quoted::span_word;

/// What a terminator says happens after the clause that ended with it ran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Terminator {
    /// `;;` — stop here (the only kind fdshell had before task #420).
    Stop,
    /// `;&` — run the next clause's body unconditionally; a forced clause's
    /// patterns are never tested, and so never expanded.
    RunNext,
    /// `;;&` — keep testing the following clauses' patterns against the same word;
    /// a retest does expand and test them.
    Retest,
}

/// The terminator at token `i` of the arm region, `None` when token `i` starts
/// none. Every word goes through [`span_word`]'s raw-span gate (task #336), so a
/// quoted `";"` / `"&"` pair is body data and terminates nothing.
pub(crate) fn kind_at(region: &[Token], line: &[u8], i: usize) -> Option<Terminator> {
    let head = region.get(i)?;
    let next = region.get(i + 1)?;
    if !semi_byte(head, line) {
        return None;
    }
    // `;&` — the `&` is a word token glued to the `;`.
    if span_word(next, b"&") && glued(head, next) {
        return Some(Terminator::RunNext);
    }
    // `;;&` — the whole triple glued; anything else behind the `;;` pair, a spaced
    // `&`, a non-glued pair or no third token at all, is the plain `;;` the walk
    // has always stopped at (its non-glued spellings included, D2).
    if span_word(next, b";") {
        let third = region.get(i + 2);
        if glued(head, next) && third.is_some_and(|t| glued(next, t) && span_word(t, b"&")) {
            return Some(Terminator::Retest);
        }
        return Some(Terminator::Stop);
    }
    None
}

/// The tokens a terminator takes — the resume step past it, added to
/// [`kind_at`]'s index (it was the hard-coded `end + 2` of `extract.rs` until the
/// third kind needed three).
pub(crate) fn tokens(kind: Terminator) -> usize {
    match kind {
        Terminator::Stop | Terminator::RunNext => 2,
        Terminator::Retest => 3,
    }
}

/// The glue test of a terminator pair: `head`'s span ends where `tail`'s begins
/// (the pair rule `h.2 == t.1`, pinned by `tests/glued_pair.rs` and the #355 pins).
pub(crate) fn glued(head: &Token, tail: &Token) -> bool {
    head.end == tail.start
}

#[cfg(test)]
mod tests;

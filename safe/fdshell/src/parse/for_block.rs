// mutants: -C --lib -- -E 'test(/^parse::for_block::/)'

pub(crate) mod arith;
mod head;
mod words;

use super::Token;
use crate::error::parse::ParseError;
use crate::keywords::Closer;
use crate::mask::MaskBit;
use crate::parse::part_text::part_text;
use crate::parse::semi::{
    List, Term, block_body, ensure_before_closer, find_preceded_by, up_to_closing,
};
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ScriptText;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct ForBlock {
    pub var: ShortCStr,
    pub words: Vec<ShortCStr>,
    /// Per-byte quote masks, parallel to `words`.
    pub words_mask: Vec<Vec<MaskBit>>,
    /// Zero-byte quote-span indices per word (task #321), parallel to
    /// `words_mask`.
    pub words_span: Vec<Vec<usize>>,
    /// Whether each word's raw span contained quotes, parallel to `words`
    /// (see `parse::word_quoted`).
    pub words_quoted: Vec<bool>,
    pub body: ScriptText,
    /// The redirections written after the closing `done` (task #298,
    /// `parse::block_tail`); they apply to the whole loop and open **before**
    /// the word list is expanded (bash's order, measured).
    pub redirects: Vec<RedirectDef>,
    /// The clause had **no** `in` (task #416), so its word list is the
    /// positional parameters at run time — `"$@"`-shaped: one word per
    /// parameter, verbatim, nothing when none are set. `words` is empty here;
    /// the empty list of `for name in; do` means zero iterations, and the flag
    /// is the whole distinction between the two rows.
    pub words_default_to_positional: bool,
}

pub(crate) fn tokens_to_for(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<ForBlock, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    // The `for` keyword, its variable name and the `in` that opens the word
    // list are the head, and their three errors are the head's contract.
    let (var, in_pos) = head::head(tokens, line)?;
    // The anchor the `do` search and the word list are measured from: the `in`
    // token index, or the name index when the clause has no `in` (task #416) —
    // the slot behind it then holds nothing but the clause terminator (or the
    // empty `newline_list`'s `do`, task #521), so `trim_semi` yields the empty
    // list (the invariant `head.rs` guards).
    let anchor = in_pos.unwrap_or(1);

    // The `for` word list takes no list terminator **and** no operator word it
    // does not own: its `do` is searched with `;` alone, and an operator word in
    // the list is rejected below (`for i in 1 & do`, `for i in 1 & ; do`,
    // `for i in 1 && ; do`, `for i in 1 | ; do` are bash syntax errors). Only
    // the **wordless** clause may leave POSIX #2.9.1's `newline_list` empty
    // before the `do` — bash runs `for i do …` at rc 0 (task #521) — so that
    // form alone searches with `SemiOrEmpty`.
    let term = if in_pos.is_some() {
        Term::Semi
    } else {
        Term::SemiOrEmpty
    };
    let do_idx =
        find_preceded_by(tokens, anchor, b"do", line, term).ok_or(ParseError::ExpectedDo)?;

    // An operator word is the async/cond/pipeline operator, not a word: bash's word
    // list ends at it and its `do`-expectation fails there (task #360 lands the plain
    // `&`, task #496 the rest of the set). The gate is the ONE predicate
    // `parse::word_quoted::operator_word`, read through the list's owned set, so a
    // quoted `"&"` / `"&&"`, an escaped `\&` and a glued `x&` stay word data. The gate
    // runs **before** the closer/body validators because bash reports the earliest
    // offending token: `for i in 1 & ; do; done` says `expected 'do'`, not `'do' body
    // is empty`.
    if words::rejects_operator_word(tokens.get(anchor + 1..do_idx).unwrap_or(&[]), line) {
        bail!(ParseError::ExpectedDo);
    }

    // A body-position heredoc's bodies sit in the block text after the closing
    // `done`: the tokens past it are body data, not block structure (task #273,
    // `semi::up_to_closing`). The `in` list is walked as `List::Words`, so a
    // closer-shaped word of the word list (`for x in done; do …; done`) is a
    // word, not the closer — and `List::Words` seeds the walk in the list
    // itself, so the **name** slot is list data too: `for done in a; do …`
    // names the variable `done` (task #522). The walk is seeded with
    // `Closer::Done`, the closer a `for` requires, so a `fi`/`esac` written in
    // the body is a word too (#309).
    let expected_done = ParseError::ExpectedDone;
    let closed = up_to_closing(tokens, line, Closer::Done, List::Words, expected_done)?;
    // The closing `done` is the truncated slice's last token.
    let done_idx = closed.len() - 1;
    let redirects =
        crate::parse::block_tail::redirects(b"done", tokens.get(done_idx..).unwrap_or(&[]), text)?;
    ensure!(
        closed
            .last()
            .is_some_and(|tok| crate::parse::block_tail::is_closer(tok, b"done")),
        ParseError::ExpectedDone
    );
    // POSIX #2.9.1: the closer is preceded by a command-list terminator — `;`
    // (a newline tokenizes as one), or the plain `&` that backgrounds the body's
    // last statement (task #354: `for i in 1; do echo hi & done`) — **or** by the
    // closer of the compound command the body ends with (task #301:
    // `for i in a; do if :; then echo f; fi done`).
    ensure_before_closer(
        closed,
        line,
        done_idx,
        ParseError::ExpectedSemicolonBeforeDone,
    )?;

    let body = part_text(
        text,
        block_body(
            closed.get(do_idx + 1..done_idx).unwrap_or(&[]),
            line,
            ParseError::EmptyDoBody,
        )?,
    )?;

    let ws = words::collect_words(closed, anchor, do_idx)?;

    Ok(ForBlock {
        var,
        words: ws.words,
        words_mask: ws.words_mask,
        words_span: ws.words_span,
        words_quoted: ws.words_quoted,
        body,
        redirects,
        words_default_to_positional: in_pos.is_none(),
    })
}

#[cfg(test)]
mod tests;

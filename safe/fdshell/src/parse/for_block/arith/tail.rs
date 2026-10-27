//! The `do … done` **tail** of the arithmetic `for`: the token-level half,
//! joined to the byte-level region scan at the region's end offset. Extracted
//! from `parse/for_block/arith.rs` for the line budget (task #456); the
//! `detect` entry that joins the two layers stays in the parent.

use crate::error::parse::ParseError;
use crate::keywords::Closer;
use crate::parse::Token;
use crate::parse::block_tail::{is_closer, redirects};
use crate::parse::part_text::part_text;
use crate::parse::semi::{List, block_body, ensure_before_closer, up_to_closing};
use crate::parse::word_quoted::{keyword_word, span_word};
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ScriptText;

/// The `do … done` tail that follows the region: token-level, joined to the byte
/// scan at the region's end offset — the tokens starting at or past it. Its
/// closing `done` also owns the block's trailing redirections (task #298).
pub(super) fn tail_body(
    raw: &[Token],
    text: &ScriptText,
    line: &[u8],
    region_end: usize,
) -> Result<(ScriptText, Vec<RedirectDef>), Report<ParseError>> {
    let tail: Vec<Token> = raw
        .iter()
        .skip_while(|(_, start, _, _, _, _)| *start < region_end)
        .cloned()
        .collect();
    // Between `))` and `do` stands at most one `;` token: `)) do`, `)) ; do`,
    // `))\ndo`, `))do`. `)) ; ; do` and `)) ;; do` are bash's rc-2 pair.
    let gap = usize::from(tail.first().is_some_and(|tok| span_word(tok, b";")));
    let tail = tail.get(gap..).unwrap_or(&[]);
    ensure!(
        tail.first()
            .is_some_and(|tok| keyword_word(tok, line, b"do")),
        ParseError::ExpectedDo
    );
    // The body's heredoc bodies sit past `done` in a single-line block, so the
    // tail is truncated at its closing keyword before anything is read (task
    // #273), with the plain list rule: the arithmetic `for` owns no word list.
    // The walk is seeded with `Closer::Done` (task #309), so a `fi`/`esac`
    // written in the body is a word and the tail rules below answer.
    let expected_done = ParseError::ExpectedDone;
    let closed = up_to_closing(tail, line, Closer::Done, List::Plain, expected_done)?;
    let done_idx = closed.len() - 1;
    let redirects = redirects(b"done", tail.get(done_idx..).unwrap_or(&[]))?;
    ensure!(
        closed.last().is_some_and(|tok| is_closer(tok, b"done")),
        ParseError::ExpectedDone
    );
    // POSIX #2.9.1, the word `for`'s rule (task #301): a terminator, or the
    // closer of the compound command the body ends with.
    ensure_before_closer(
        closed,
        line,
        done_idx,
        ParseError::ExpectedSemicolonBeforeDone,
    )?;
    let body = block_body(
        closed.get(1..done_idx).unwrap_or(&[]),
        line,
        ParseError::EmptyDoBody,
    )?;
    // Statement-relative offsets, the statement's own `ScriptText`: a body
    // heredoc's region offsets are read from this text.
    part_text(text, body).map(|body| (body, redirects))
}

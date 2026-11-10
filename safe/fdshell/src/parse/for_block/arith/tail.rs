// mutants: -C --lib -- -E 'test(/^parse::for_block::/)'

//! The `do … done` **tail** of the arithmetic `for`: the token-level half,
//! joined to the byte-level region scan at the region's end offset. Extracted
//! from `parse/for_block/arith.rs` for the line budget (task #456); the
//! `detect` entry that joins the two layers stays in the parent.

use crate::error::parse::ParseError;
use crate::keywords::Closer;
use crate::parse::Token;
use crate::parse::block_tail::{is_closer, redirects};
use crate::parse::group_block::tokens_to_group;
use crate::parse::part_text::part_text;
use crate::parse::semi::{List, block_body, ensure_before_closer, up_to_closing};
use crate::parse::word_quoted::keyword_word;
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
        .skip_while(|Token { start, .. }| *start < region_end)
        .cloned()
        .collect();
    // Between the region and the body keyword (the `do`, or the `{` of a group
    // body) stands at most one `;` token and then any run of newlines: `)) do`,
    // `)) ; do`, `))\ndo`, `))do`, `))\n\ndo`, `)) ;\ndo`. `)) ; ; do` and
    // `)) ;; do` are bash's rc-2 pair.
    let gap = gap_len(&tail, line);
    let tail = tail.get(gap..).unwrap_or(&[]);
    // The **brace body** (task #394): bash's `for_action` ends with a compound
    // list, so a group opener standing behind the region is the loop's body. The
    // group parse answers it — its terminator rule, its emptiness rule, its
    // trailing redirections and its quote gates are the group's own, not a copy.
    if tail
        .first()
        .is_some_and(|tok| keyword_word(tok, line, b"{"))
    {
        let group = tokens_to_group(tail, text)?;
        return Ok((group.body, group.redirects));
    }
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
    let redirects = redirects(b"done", tail.get(done_idx..).unwrap_or(&[]), text)?;
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

/// The gap in front of the body keyword: **at most one** real `;` byte, then
/// any run of newlines. Bash's grammar gives this tail one
/// `simple_list_terminator` slot and absorbs the newlines behind it
/// (`)) ;\ndo`, `))\n\ndo`), while a `;` behind a newline, a second `;`
/// (`; ;`, `;;`) and a `&` stay parse errors — bash's rc-2 family, measured.
fn gap_len(tail: &[Token], line: &[u8]) -> usize {
    let mut run = 0;
    if separator_byte(tail, line, run, b';') {
        run += 1;
    }
    while separator_byte(tail, line, run, b'\n') {
        run += 1;
    }
    run
}

/// The `;` token at `idx` of `tail` is the separator byte `byte` in `line`: the
/// tokenizer folds a `;` and a newline into one `;` token
/// (`parse/token/step/sep.rs`), so only the raw byte tells them apart — the
/// test `semi/preceded.rs::amp_semi` reads for the same pair. The [`Token::span_word`]
/// gate keeps a quoted `";"` word (and an escaped `\;`) out of the run.
fn separator_byte(tail: &[Token], line: &[u8], idx: usize, byte: u8) -> bool {
    let Some(tok @ Token { start, .. }) = tail.get(idx) else {
        return false;
    };
    tok.span_word(b";") && line.get(*start) == Some(&byte)
}

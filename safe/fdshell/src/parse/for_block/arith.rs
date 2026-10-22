//! The arithmetic `for ((init;cond;step)) ; do … ; done` — bash's `FOR_ACTION`,
//! recognized on the **raw bytes**: the tokenizer emits `)` as a separator token
//! and reads `|` as a pipe, so the three clauses are unrecoverable at the token
//! level (the rule [`super::super::arith_cmd`] records for `((expr))`).
//! The `do … done` tail *is* token-level; the two layers join by byte offset.

mod region;

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::part_text::part_text;
use crate::parse::semi::{List, block_body, is_list_terminator, up_to_closing};
use crate::parse::word_quoted::keyword_word;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;

/// The arithmetic `for`: its three clauses — `None` for each one written empty —
/// and the `do … done` body.
#[cfg_attr(test, derive(Debug))]
pub struct ArithForBlock {
    pub init: Option<ScriptText>,
    pub cond: Option<ScriptText>,
    pub step: Option<ScriptText>,
    pub body: ScriptText,
    /// The redirections written after the closing `done` (task #298,
    /// `parse::block_tail`); they apply to the whole loop.
    pub redirects: Vec<RedirectDef>,
}

/// `Some(block)` when `raw`'s `for` keyword word opens an `(( … ))` region;
/// `Ok(None)` when the statement is the word form, so the caller keeps it.
/// The `for` word itself is gated by [`keyword_word`] in `parse/dispatch`, so a
/// quoted `'for'` never reaches this parser.
pub(crate) fn detect(
    raw: &[Token],
    text: &ScriptText,
) -> Result<Option<ArithForBlock>, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let Some(open) = region::open_at(raw, line) else {
        return Ok(None);
    };
    let Some(region) = region::measure(line, open)? else {
        return Ok(None);
    };
    let [init, cond, step] = region::clauses(text, &region)?;
    let (body, redirects) = tail_body(raw, text, line, region.end)?;
    Ok(Some(ArithForBlock {
        init,
        cond,
        step,
        body,
        redirects,
    }))
}

/// The `do … done` tail that follows the region: token-level, joined to the byte
/// scan at the region's end offset — the tokens starting at or past it. Its
/// closing `done` also owns the block's trailing redirections (task #298).
fn tail_body(
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
    let gap = usize::from(
        tail.first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b";")),
    );
    let tail = tail.get(gap..).unwrap_or(&[]);
    ensure!(
        tail.first()
            .is_some_and(|tok| keyword_word(tok, line, b"do")),
        ParseError::ExpectedDo
    );
    // The body's heredoc bodies sit past `done` in a single-line block, so the
    // tail is truncated at its closing keyword before anything is read (task
    // #273), with the plain list rule: the arithmetic `for` owns no word list.
    let closed = up_to_closing(tail, line, List::Plain, ParseError::ExpectedDone)?;
    let done_idx = closed.len() - 1;
    let redirects =
        crate::parse::block_tail::redirects(b"done", tail.get(done_idx..).unwrap_or(&[]), line)?;
    ensure!(
        closed
            .last()
            .is_some_and(|tok| crate::parse::block_tail::is_closer(tok, b"done")),
        ParseError::ExpectedDone
    );
    ensure!(
        closed.get(done_idx - 1).is_some_and(is_list_terminator),
        ParseError::ExpectedSemicolonBeforeDone
    );
    let body = block_body(
        closed.get(1..done_idx).unwrap_or(&[]),
        line,
        ParseError::EmptyDoBody,
    )?;
    // Statement-relative offsets, the statement's own `ScriptText`: a body
    // heredoc's region offsets are read from this text.
    part_text(text, body).map(|body| (body, redirects))
}

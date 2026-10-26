//! The arithmetic `for ((init;cond;step)) ; do … ; done` — bash's `FOR_ACTION`,
//! recognized on the **raw bytes**: the tokenizer emits `)` as a separator token
//! and reads `|` as a pipe, so the three clauses are unrecoverable at the token
//! level (the rule [`super::super::arith_cmd`] records for `((expr))`).
//! The `do … done` tail *is* token-level; the two layers join by byte offset.

mod region;
mod tail;

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use tail::tail_body;

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

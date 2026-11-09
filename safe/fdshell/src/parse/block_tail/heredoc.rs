// mutants: -C --lib -C --test -C block_bodies -C --test -C block_closers -C --test -C block_conditions -C --test -C block_redirect -- -E 'test(/^parse::block_tail::/) | binary(block_bodies) | binary(block_closers) | binary(block_conditions) | binary(block_redirect)'

//! The **here-doc** arm of the keyword block's tail walk (task #408): a `<<`,
//! `<<-` or `N<<` operator written after the closing keyword belongs to the whole
//! block, so its body region is claimed at **slice** granularity — the operators
//! written on the same physical line **before** the tail slice already took their
//! regions (`parse::cond_bodies`' window/skip pair, the invariant of LESSONS "A
//! heredoc body is a property of the logical line, not of a contiguous span").
//! The body itself — its bytes, the `<<-` strip, the quote flag, the end-of-file
//! shape and its warning — is `parse::heredoc`'s rule, shared with the
//! simple-command level, so this module adds a granularity and no second reading.

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::heredoc::body_for;
use crate::parse::semi::token_range;
use crate::redirect::RedirectDef;
use crate::scan::heredoc::{
    attached, body_spans, line_end_after, line_start, operator_delims, operator_sites,
    span_operator,
};
use crate::warn;
use error_stack::{Report, ResultExt, bail};
use sys::ScriptText;

/// The here-doc redirect of the tail token at `i` and the number of following
/// tokens its operator consumes (1 for the bare `<< word` form, whose delimiter is
/// the next word) — the contract `parse::redirect::parse_redirect` gives the walk.
/// `Ok(None)` means the token is not this arm's: the shape test is
/// [`span_operator`]'s raw-span rule, and the site must be one the **byte scan**
/// counts on the operator's physical line — a `<<` glued inside a word (`done<<T`)
/// opens no word, so it stays word data and the walk's other arms answer it.
pub(crate) fn def_at(
    text: &ScriptText,
    tail: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let Token { start, end, .. } = tail.get(i).ok_or(ParseError::Never)?;
    let Some((site, _prefix)) = span_operator(line, *start, *end) else {
        return Ok(None);
    };
    // The window and the site list are one pair (`scan::heredoc::ops`: one entry
    // per operator of the window, in operator order), so a site listed there owns
    // the region at the same index of the line's body allocation — the window/skip
    // pair `parse::cond_bodies` applies to an arm slice, and the slice start is
    // where the operators written before it stop belonging to it.
    let from = token_range(tail).0;
    let anchor = line_start(line, from);
    let line_end = line_end_after(line, from);
    let Some(rank) = operator_sites(line, anchor, line_end)
        .iter()
        .position(|&s| s == site)
    else {
        return Ok(None);
    };
    let ops = operator_delims(line, anchor, line_end).unwrap_or_default();
    let Some(op) = ops.get(rank) else {
        // A site the byte scan counts but cannot read — a bare `<<` with no
        // delimiter word. The parse layer owns that shape and its answer is the
        // house redirect error (bash: rc 2), the same one `operator_at` gives at
        // the simple-command level.
        bail!(ParseError::InvalidRedirect);
    };
    let spans = body_spans(line, line_end, &ops).0;
    let span = spans.get(rank).ok_or(ParseError::Never)?;
    let spec = body_for(op, span, line)?;
    if !span.found {
        // Task #364's rule and its existing wording: the delimiter line never
        // arrived, so the claim covers what the stream left and the parse layer
        // prints the end-of-file warning for the operators it decides for — here,
        // the tail slice's own, never an operator before it on the line.
        warn::here_doc_eof(&op.delim);
    }
    let extra = usize::from(!attached(line, site, *end));
    Ok(Some((
        RedirectDef::here_doc(op.fd, spec.body, spec.expand),
        extra,
    )))
}

#[cfg(test)]
mod tests;

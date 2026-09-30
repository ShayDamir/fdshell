//! Pre-expansion of `$(…)` and nested `$((…))` inside a `$((…))` body.
//!
//! The raw body bytes are walked once and each substitution is spliced in
//! place with its result, so the remainder is a single arithmetic expression
//! the lex/parse/eval pipeline can consume (bash's model: expand first, then
//! evaluate the resulting string).

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// Expand `$(…)` command substitutions and nested `$((…))` in a `$((…))` body,
/// splicing each result in place. All other bytes (including other `$` forms,
/// which stay arithmetic syntax errors) are copied verbatim.
pub(super) fn expand_body(
    body: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<u8>, Report<ResolveError>> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = body.get(i) {
        i += 1;
        if c == b'$' && body.get(i) == Some(&b'(') {
            i = splice_dollar_paren(body, i, cell, &mut out)?;
            continue;
        }
        out.push(c);
    }
    Ok(out)
}

/// Splice the `$(…)` or `$((…))` whose opening `(` is at `body[open]` into
/// `out`, returning the index just past the construct.
fn splice_dollar_paren(
    body: &[u8],
    open: usize,
    cell: &ForkCell<ShellState>,
    out: &mut Vec<u8>,
) -> Result<usize, Report<ResolveError>> {
    if body.get(open + 1) == Some(&b'(') {
        return splice_arith(body, open + 2, cell, out);
    }
    let Some((inner, close)) = scan_at(body, open + 1, 1) else {
        bail!(ResolveError::UnclosedParen);
    };
    let captured =
        crate::cmd_subst::run_and_capture(&inner, cell).change_context(ResolveError::ArithSubst)?;
    out.extend_from_slice(&captured);
    Ok(close)
}

/// Splice a nested `$((…))` whose inner body starts at `start`, consuming both
/// closers. The result is the nested expression's decimal value.
fn splice_arith(
    body: &[u8],
    start: usize,
    cell: &ForkCell<ShellState>,
    out: &mut Vec<u8>,
) -> Result<usize, Report<ResolveError>> {
    let Some((inner, close)) = scan_at(body, start, 2) else {
        bail!(ResolveError::UnclosedParen);
    };
    // The scan's `)` is the inner closer; the outer closer must follow it,
    // else the `$((` is not fully closed.
    ensure!(body.get(close) == Some(&b')'), ResolveError::UnclosedParen);
    let value = crate::nest::deeper(cell, ResolveError::ArithTooDeep, || {
        super::eval(&inner, cell)
    })?;
    let text = super::render(value)?;
    let bytes = text.as_bytes().change_context(ResolveError::Never)?;
    out.extend_from_slice(bytes);
    Ok(close + 1)
}

/// Scan a parenthesized body from `start` (just past the opening paren(s) the
/// caller consumed), tracking quote state like `paren_scan::scan_paren_body`.
/// `depth` is how many parens the caller already consumed; the body ends at the
/// first `)` bringing the count back to `depth`. Returns the body without that
/// final `)` and the index just past it, or `None` if the input ends first.
fn scan_at(body: &[u8], start: usize, depth: u32) -> Option<(Vec<u8>, usize)> {
    let mut out = Vec::new();
    let mut d = depth;
    let mut in_quotes = false;
    let mut i = start;
    while let Some(&c) = body.get(i) {
        i += 1;
        if in_quotes && c == b'\\' {
            let escaped = *body.get(i)?;
            i += 1;
            out.push(b'\\');
            out.push(escaped);
            continue;
        }
        if c == b')' && !in_quotes && d == depth {
            return Some((out, i));
        }
        out.push(c);
        match c {
            b'"' => in_quotes = !in_quotes,
            b'(' if !in_quotes => d += 1,
            b')' if !in_quotes => d -= 1,
            _ => {}
        }
    }
    None
}

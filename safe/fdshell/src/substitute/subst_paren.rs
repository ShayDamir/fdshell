//! `$(…)` in the substitution pass: `$(cmd)` runs a child and captures its
//! output; `$((expr))` is evaluated in-process as an arithmetic expression.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::mask::{Counting, push_expanded};
use crate::error::resolve::ResolveError;
use crate::paren_scan::scan_paren_body;
use crate::state::ShellState;

/// The caller consumed `$` and the first `(`; this handles the rest.
pub(super) fn handle_dollar_paren<I: Iterator<Item = u8>>(
    peek: &mut core::iter::Peekable<Counting<I>>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    out_mask: &mut Vec<bool>,
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    if peek.peek() == Some(&b'(') {
        return arith_body(peek, cell, out, out_mask, quoted);
    }
    let inner = super::paren::read_paren_expr(peek)?;
    let expanded =
        crate::cmd_subst::run_and_capture(&inner, cell).change_context(ResolveError::Resolve)?;
    push_expanded(out, out_mask, &expanded, quoted)
}

/// `$((…))`: the body scan starts at depth 2 — the inner `(` and the `)` that
/// re-enters depth 2 stay inside the body (grouping the parser understands).
fn arith_body<I: Iterator<Item = u8>>(
    peek: &mut core::iter::Peekable<Counting<I>>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    out_mask: &mut Vec<bool>,
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    let Some(body) = scan_paren_body(peek, 2) else {
        bail!(ResolveError::UnclosedParen);
    };
    let value = crate::arith::eval(&body, cell)?;
    let text = crate::arith::render(value)?;
    // A rendered number is always valid byte content.
    let bytes = text.as_bytes().change_context(ResolveError::Never)?;
    push_expanded(out, out_mask, bytes, quoted)
}

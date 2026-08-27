//! Single-argument substitution — handles ~, %, $(), and $.

use alloc::vec::Vec;
use core::cell::Cell;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::mask::{Counting, push_byte, push_expanded, realign};
use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// Substitute one word, returning the expanded text plus a parallel quote
/// mask. Bytes copied from the word keep their mask bit; bytes produced by
/// an expansion inherit the mask bit of the expansion's trigger byte.
///
/// Invariant: at the top of each loop iteration `idx == consumed.get()`.
pub(crate) fn substitute_arg(
    arg: &ShortCStr,
    mask: &[bool],
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(ShortCStr, Vec<bool>), Report<ResolveError>> {
    let bytes = arg.as_bytes().change_context(ResolveError::RefNotFound)?;
    let mut out = ShortCStr::new();
    let mut out_mask = Vec::new();
    let consumed = Cell::new(0usize);
    let mut peek = Counting {
        inner: bytes.iter().copied(),
        consumed: &consumed,
    }
    .peekable();
    let mut idx = 0usize;
    if bytes.first() == Some(&b'~') {
        super::tilde::expand(&mut peek, &mut idx, mask, &mut out, &mut out_mask)?;
    }
    while let Some(b) = peek.next() {
        let quoted = mask.get(idx).copied().unwrap_or(false);
        idx += 1;
        match b {
            // `\$` and `\\` drop the backslash; any other `\<char>` keeps it.
            b'\\' => match peek.peek() {
                Some(&c @ (b'$' | b'\\')) => {
                    peek.next();
                    idx += 1;
                    push_byte(&mut out, &mut out_mask, c, quoted)?;
                }
                _ => push_byte(&mut out, &mut out_mask, b'\\', quoted)?,
            },
            b'%' => {
                let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
                let before = out.len();
                crate::substitute::percent::percent_subst(&mut peek, cache, &state, &mut out)?;
                realign(
                    &mut idx,
                    &consumed,
                    &mut out_mask,
                    before,
                    out.len(),
                    quoted,
                );
            }
            b'$' if peek.peek() == Some(&b'(') => {
                peek.next();
                let inner = crate::substitute::paren::read_paren_expr(&mut peek)?;
                let expanded = crate::cmd_subst::run_and_capture(&inner, cell)
                    .change_context(ResolveError::Resolve)?;
                push_expanded(&mut out, &mut out_mask, &expanded, quoted)?;
                idx = consumed.get();
            }
            b'$' => {
                let before = out.len();
                crate::substitute::dollar::dollar_subst(&mut peek, cell, &mut out)?;
                realign(
                    &mut idx,
                    &consumed,
                    &mut out_mask,
                    before,
                    out.len(),
                    quoted,
                );
            }
            _ => push_byte(&mut out, &mut out_mask, b, quoted)?,
        }
    }
    Ok((out, out_mask))
}

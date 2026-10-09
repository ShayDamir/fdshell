//! Single-argument substitution — handles ~, %, $(), $((…)), and $.

mod arms;

use alloc::vec::Vec;
use core::cell::Cell;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::mask::{Counting, push_byte, realign};
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
            // Unquoted POSIX #4.1: `\X` removes the backslash and the escaped
            // byte becomes protected (mask `true` — it never splits or globs).
            // `\<newline>` is a line continuation: both bytes are dropped. A
            // trailing `\` at end of word keeps the backslash.
            b'\\' if !quoted => match peek.next() {
                Some(b'\n') => {
                    idx += 1;
                }
                Some(c) => {
                    idx += 1;
                    push_byte(&mut out, &mut out_mask, c, true)?;
                }
                None => push_byte(&mut out, &mut out_mask, b'\\', false)?,
            },
            // Quoted POSIX #4.2: `\$` and `\\` drop the backslash; any other
            // `\<char>` keeps it.
            b'\\' => match peek.peek() {
                Some(&c @ (b'$' | b'\\')) => {
                    peek.next();
                    idx += 1;
                    push_byte(&mut out, &mut out_mask, c, quoted)?;
                }
                _ => push_byte(&mut out, &mut out_mask, b'\\', quoted)?,
            },
            b'%' => {
                let before = out.len();
                arms::percent(&mut peek, cache, cell, &mut out)?;
                realign(&mut idx, &consumed, &mut out_mask, before, &out, quoted);
            }
            b'$' if peek.peek() == Some(&b'(') => {
                peek.next();
                super::subst_paren::handle_dollar_paren(
                    &mut peek,
                    cell,
                    &mut out,
                    &mut out_mask,
                    quoted,
                )?;
                idx = consumed.get();
            }
            b'$' => {
                let before = out.len();
                arms::dollar(&mut peek, cell, &mut out)?;
                realign(&mut idx, &consumed, &mut out_mask, before, &out, quoted);
            }
            _ => push_byte(&mut out, &mut out_mask, b, quoted)?,
        }
    }
    Ok((out, out_mask))
}

//! Single-argument substitution — handles ~, %, $(), `` ` … ` ``, $((…)), and $.

mod arms;
mod escape;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::Join;
use super::dollar::Region;
use super::mask::{Counting, push_byte, realign};
use super::origin::Origin;
use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::mask::quoted_at;
use crate::mask::region_len;
use crate::state::ShellState;

/// Substitute one word, returning the expanded text, a parallel quote mask
/// and the byte provenance. Bytes copied from the word keep their mask bit;
/// bytes produced by an expansion inherit the mask bit of the expansion's
/// trigger byte. `Origin` records which output bytes came from an expansion
/// (field splitting cuts only those, POSIX 2.6.5) and where an embedded `$@`
/// positional starts a new word; the tilde arm records nothing — bash never
/// splits the expanded `$HOME` (task #222). `spans` is the word's list of
/// zero-byte quote-span indices (task #321), input-side only: it feeds the
/// tilde gate and never affects the output mask. `join` is the `$@`/`$*` arm's
/// join mode — the caller's surface, not a flag read off the state (task #319).
///
/// Invariant: at the top of each loop iteration `idx == peek.consumed()`.
pub(super) fn substitute_arg_marked(
    arg: &ShortCStr,
    mask: &[MaskBit],
    spans: &[usize],
    join: Join,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(ShortCStr, Vec<MaskBit>, Origin), Report<ResolveError>> {
    let bytes = arg.as_bytes().change_context(ResolveError::RefNotFound)?;
    let mut out = ShortCStr::new();
    let mut out_mask = Vec::new();
    let mut origin = Origin::new();
    let mut peek = Counting::new(bytes.iter().copied());
    let mut idx = 0usize;
    if super::tilde::leads(bytes, mask, spans) {
        super::tilde::expand(&mut peek, &mut idx, mask, cell, &mut out, &mut out_mask)?;
    }
    while let Some(b) = peek.next() {
        let quoted = quoted_at(mask, idx);
        // A name arm (`$name`, `%name`, `$N`) reads at most the quote region of
        // its own trigger byte: `idx` is that byte's word index here, before
        // the arm's peek-ahead moves it.
        let limit = region_len(mask, idx).saturating_sub(1);
        idx += 1;
        match b {
            b'\\' if !quoted => escape::unquoted(&mut peek, &mut idx, &mut out, &mut out_mask)?,
            b'\\' => escape::quoted(&mut peek, &mut idx, &mut out, &mut out_mask, quoted)?,
            // `%name` fd substitution. The arm consumes the trigger byte and
            // at most the name; a following `%` or non-name byte is literal,
            // so `%%` reaches argv intact — the `printf` builtin owns `%%`
            // (task #155). `realign` keeps the mask 1:1 for the literal `%`.
            b'%' => {
                let before = out.len();
                arms::percent(&mut peek, cache, cell, &mut out, limit)?;
                realign(&mut idx, &peek, &mut out_mask, before, &out, quoted);
                origin.record(before, out.len());
            }
            b'$' if peek.peek() == Some(&b'(') => {
                let before = out.len();
                peek.next();
                super::subst_paren::handle_dollar_paren(
                    &mut peek,
                    cell,
                    &mut out,
                    &mut out_mask,
                    quoted,
                )?;
                idx = peek.consumed();
                origin.record(before, out.len());
            }
            // `` ` … ` `` runs the same capture machinery as `$( … )`; the
            // trigger byte's mask bit protects the output from splitting.
            b'`' => {
                let before = out.len();
                super::backtick::handle(&mut peek, cell, &mut out, &mut out_mask, quoted)?;
                idx = peek.consumed();
                origin.record(before, out.len());
            }
            b'$' => {
                let before = out.len();
                // `idx` is the word index of the byte after `$` — the `{` of a
                // `${…}` arm; `realign` re-syncs it to the consumed count.
                arms::dollar(
                    &mut peek,
                    cell,
                    &mut out,
                    Region {
                        mask,
                        start: idx,
                        limit,
                    },
                    quoted,
                    join,
                    &mut origin,
                )?;
                realign(&mut idx, &peek, &mut out_mask, before, &out, quoted);
                origin.record(before, out.len());
            }
            _ => push_byte(&mut out, &mut out_mask, b, quoted)?,
        }
    }
    Ok((out, out_mask, origin))
}

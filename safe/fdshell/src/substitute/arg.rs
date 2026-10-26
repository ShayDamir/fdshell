//! Single-argument substitution — handles ~, %, $(), `` ` … ` ``, $((…)), and $.
//!
//! The loop's whole state is [`Ex`] (in `arg/state.rs`), and every arm of the
//! loop is an `impl Ex` block in `arg/arms.rs`: the arm owns its bookkeeping
//! (the mask realign, the `idx` re-sync, the `origin` record), so the loop body
//! here is one call per byte.

mod arms;
mod escape;
mod state;

pub(super) use state::Ex;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::Join;
use super::mask::Counting;
use super::origin::Origin;
use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
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
    let mut ex = Ex {
        peek: Counting::new(bytes.iter().copied()),
        idx: 0,
        out: ShortCStr::new(),
        out_mask: Vec::new(),
        origin: Origin::new(),
        cell,
        cache,
        mask,
        join,
    };
    if super::tilde::leads(bytes, mask, spans) {
        super::tilde::expand(
            &mut ex.peek,
            &mut ex.idx,
            mask,
            cell,
            &mut ex.out,
            &mut ex.out_mask,
        )?;
    }
    while let Some(b) = ex.peek.next() {
        ex.dispatch(b)?;
    }
    Ok((ex.out, ex.out_mask, ex.origin))
}

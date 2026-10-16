//! The `%` and `$` arms of the single-argument substitution loop: run the
//! substitution (the `%` arm borrows the state first) and hand the output
//! back for the caller's mask realign.

use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// The `%` arm: the state borrow ends before the caller realigns the mask.
/// `limit` is the name cap of the trigger byte's quote region.
pub(super) fn percent(
    peek: &mut crate::substitute::mask::Counting<'_>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    limit: usize,
) -> Result<(), Report<ResolveError>> {
    let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
    crate::substitute::percent::percent_subst(peek, cache, &state, out, limit)
}

/// The `$` arm (the `$(` arm is handled separately in the loop). `region`
/// carries the mask, the word index of the byte after `$`, and the name cap
/// of the trigger's quote region; `quoted` is the trigger byte's mask bit,
/// the quote bit the `$@` arm gives its positionals; `origin` records the
/// arm's positional boundaries.
pub(super) fn dollar(
    peek: &mut crate::substitute::mask::Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    region: crate::substitute::dollar::Region<'_>,
    quoted: bool,
    origin: &mut crate::substitute::origin::Origin,
) -> Result<(), Report<ResolveError>> {
    crate::substitute::dollar::dollar_subst(peek, cell, out, region, quoted, origin)
}

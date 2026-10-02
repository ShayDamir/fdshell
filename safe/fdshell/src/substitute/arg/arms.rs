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
pub(super) fn percent(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
    crate::substitute::percent::percent_subst(peek, cache, &state, out)
}

/// The `$` arm (the `$(` arm is handled separately in the loop).
pub(super) fn dollar(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    crate::substitute::dollar::dollar_subst(peek, cell, out)
}

//! The two **name-shaped** arms of the `$` arm (task #439): `$<digit…>` and
//! `$<name>`. Both are name-shaped references, and both are capped by the
//! trigger byte's quote region — the cap is `dollar_subst`'s own
//! `region_ends_at_dollar`, run **before** this file is entered and **before**
//! the trigger byte is consumed, so a region that ends at the `$` writes the
//! literal `$` and consumes nothing. Keeping the cap in the caller is what makes
//! it cover every arm of the `$` arm, and the LESSONS entry
//! "`dollar_subst`'s digit arm is load-bearing" names this file's digit arm.

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::mask::Counting;
use crate::substitute::{borrow_state, percent, resolve};
use error_stack::Report;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// `$0` is the program name; `$1, $2, ... $N` are the positionals. The digit
/// run is capped at the trigger byte's quote region exactly as a name is, so a
/// region that ends at the `$` leaves no digit (the caller's cap wrote the
/// literal `$` and never got here): the digits reach the loop as literal bytes —
/// the `$"…"` family (`$"1"` is `$1`, bash `1`; `"$"12` is `$12`, bash parity).
///
/// `c` is the first digit, which the caller matched on and this arm hands to
/// [`resolve::resolve_positional_index`] as the run's start. `limit` is the
/// region's remaining bytes, at least one.
pub(super) fn positional(
    c: u8,
    peek: &mut Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    limit: usize,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    let state = borrow_state(cell)?;
    resolve::resolve_positional_index(c, peek, &state, out, limit)
}

/// A name never crosses out of its trigger byte's quote region (`"$v"bc` is the
/// parameter `v` plus the literal `bc`), so a region that ends at the `$` leaves
/// no name and the `$` is literal — the caller's cap, the same rule `$'…'` and
/// `$"…"` already follow. `limit` is the region's remaining bytes, at least one.
pub(super) fn variable(
    peek: &mut Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    limit: usize,
) -> Result<(), Report<ResolveError>> {
    let name_scs = percent::collect_name(peek, limit)?;
    let state = borrow_state(cell)?;
    if name_scs.eq_bytes(b"_") {
        // `$_`: the `_` variable set by the shell after each command.
        // Empty when unset, like every other parameter; `$_` is not
        // nounset-checked (accepted divergence: bash never reports `_`
        // unbound in `-c` mode, because `set -u` itself binds it).
        if let Some(val) = state.var_value(&name_scs) {
            out.push(val);
        }
    } else {
        state.resolve_var_name(&name_scs, out)?;
    }
    Ok(())
}

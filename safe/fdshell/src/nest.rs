// mutants: -C --lib -- -E 'test(/^nest::/)'

use error_stack::{Report, ResultExt, bail, ensure};
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

/// Maximum nesting depth of blocks (`if`/`while`/`until`/`for`/`case`), the
/// `{ …; }` group, command substitutions, and nested `$((…))` arithmetic.
/// Without a cap, each level re-scans and re-parses the remaining body, making
/// script execution O(n^2) in script size.
pub(crate) const MAX_NESTING: u32 = 100;

/// Run `f` one nesting level deeper than the current one.
///
/// Fails with `limit` if entering the level would exceed [`MAX_NESTING`] or
/// if the state cannot be borrowed (an internal bug at these call sites).
/// The depth is restored before `f`'s result is returned.
///
/// Stays a free function (task #567): the receiver is `sys`'s `ForkCell`, and
/// Rust forbids an inherent impl on a type from another crate even when a
/// generic argument is local (E0116) — the receiver's own crate owns its
/// inherent methods.
pub(crate) fn deeper<E, T, F>(cell: &ForkCell<ShellState>, limit: E, f: F) -> Result<T, Report<E>>
where
    E: core::error::Error + Send + Sync + 'static,
    F: FnOnce() -> Result<T, Report<E>>,
{
    let Ok(mut state) = cell.borrow_mut() else {
        bail!(limit);
    };
    ensure!(state.nesting < MAX_NESTING, limit);
    state.nesting += 1;
    drop(state);
    let result = f();
    let mut state = cell.borrow_mut().change_context(limit)?;
    state.nesting -= 1;
    result
}

#[cfg(test)]
mod tests;

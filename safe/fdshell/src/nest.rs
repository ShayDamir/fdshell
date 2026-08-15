use error_stack::{Report, ResultExt, bail, ensure};
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

/// Maximum nesting depth of blocks (`if`/`while`/`until`/`for`/`case`) and
/// command substitutions. Without a cap, each level re-scans and re-parses
/// the remaining body, making script execution O(n^2) in script size.
pub(crate) const MAX_NESTING: u32 = 100;

/// Run `f` one nesting level deeper than the current one.
///
/// Fails with `limit` if entering the level would exceed [`MAX_NESTING`] or
/// if the state cannot be borrowed (an internal bug at these call sites).
/// The depth is restored before `f`'s result is returned.
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

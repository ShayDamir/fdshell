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
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::error::cmd::CmdError;

    fn make_cell() -> ForkCell<ShellState> {
        ForkCell::new(ShellState::new())
    }

    #[test]
    fn deeper_restores_depth_on_error() {
        let cell = make_cell();
        let result: Result<(), Report<CmdError>> = deeper(&cell, CmdError::NestingTooDeep, || {
            Err(Report::new(CmdError::Invalid))
        });
        assert!(result.is_err());
        assert_eq!(cell.borrow().unwrap().nesting, 0);
    }

    #[test]
    fn deeper_restores_depth_on_success() {
        let cell = make_cell();
        let value = deeper(&cell, CmdError::NestingTooDeep, || {
            let inner = deeper(&cell, CmdError::NestingTooDeep, || {
                assert_eq!(cell.borrow().unwrap().nesting, 2);
                Ok(7)
            })
            .unwrap();
            Ok(inner)
        })
        .unwrap();
        assert_eq!(value, 7);
        assert_eq!(cell.borrow().unwrap().nesting, 0);
    }

    #[test]
    fn deeper_enforces_limit() {
        let cell = make_cell();
        {
            let mut state = cell.borrow_mut().unwrap();
            state.nesting = MAX_NESTING;
        }
        let result = deeper(&cell, CmdError::NestingTooDeep, || Ok(1));
        assert!(matches!(
            result.unwrap_err().current_context(),
            CmdError::NestingTooDeep
        ));
        assert_eq!(cell.borrow().unwrap().nesting, MAX_NESTING);
    }
}

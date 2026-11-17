//! Applying the expanded prefix to the shell state, and restoring it.

// mutants: -C --lib -- -E 'test(/^run_env::/)'

mod save;

pub use save::{EnvSave, restore};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use super::EnvAssign;

/// Parent-side (functions, intercepts): apply now, return the previous
/// values for `restore`.
pub fn apply(
    env: &[EnvAssign],
    cell: &ForkCell<ShellState>,
) -> Result<Option<EnvSave>, Report<CmdError>> {
    if env.is_empty() {
        return Ok(None);
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let mut save = EnvSave {
        entries: Vec::new(),
        old_ifs: None,
        old_optind: None,
    };
    for a in env {
        save::record(&mut save, &mut state, &a.name);
        state.set_var(a.name.clone(), a.value.clone());
        state.exports.insert(a.name.clone(), a.value.clone());
    }
    Ok(Some(save))
}

/// Fork-side: apply in the forked child after arg substitution; the child
/// exits, so no restore.
pub fn apply_child(env: &[EnvAssign], cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    if env.is_empty() {
        return Ok(());
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    for a in env {
        state.set_var(a.name.clone(), a.value.clone());
        state.exports.insert(a.name.clone(), a.value.clone());
    }
    Ok(())
}

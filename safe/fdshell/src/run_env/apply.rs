//! Applying the expanded prefix to the shell state, and restoring it.

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ShortCStr};

use super::EnvAssign;

/// The previous values of the first-touched names, plus the old `IFS` when
/// the prefix sets one: `set_var` syncs `state.ifs`, but IFS is not seeded
/// in `strings`, so a scoped IFS would leak into the parent on restore.
pub struct EnvSave {
    entries: Vec<(ShortCStr, Option<ImportedStr>, Option<ImportedStr>)>,
    old_ifs: Option<ShortCStr>,
}

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
    };
    for a in env {
        record(&mut save, &mut state, &a.name);
        state.set_var(a.name.clone(), a.value.clone());
        state.exports.insert(a.name.clone(), a.value.clone());
    }
    Ok(Some(save))
}

/// Put the previous values back, in first-touch order.
pub fn restore(save: Option<EnvSave>, cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let Some(save) = save else {
        return Ok(());
    };
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    for (name, old_strings, old_exports) in save.entries {
        match old_strings {
            Some(value) => state.set_var(name.clone(), value),
            None => {
                let _ = state.strings.remove(&name);
            }
        }
        match old_exports {
            Some(value) => {
                let _ = state.exports.insert(name.clone(), value);
            }
            None => {
                let _ = state.exports.remove(&name);
            }
        }
    }
    if let Some(ifs) = save.old_ifs {
        state.ifs = ifs;
    }
    Ok(())
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

/// Record the previous values of `name` on first touch only, so
/// `FOO=1 FOO=2 cmd` restores the original `FOO`.
fn record(save: &mut EnvSave, state: &mut ShellState, name: &ShortCStr) {
    if save.entries.iter().any(|(n, _, _)| n == name) {
        return;
    }
    let old_strings = state.strings.get(name).cloned();
    let old_exports = state.exports.get(name).cloned();
    if name.eq_bytes(b"IFS") {
        save.old_ifs = Some(state.ifs.clone());
    }
    save.entries.push((name.clone(), old_strings, old_exports));
}

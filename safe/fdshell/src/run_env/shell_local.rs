//! The store split of a `NAME=value` prefix on a command whose name is
//! omitted: the shell's variable list adjusts, the execution environment does
//! not (POSIX 2.9.1).

use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

use super::EnvSave;

/// Keep everything the apply put into the shell's own store (the `strings`
/// entries and the `IFS` field `set_var` synced) and put back every export the
/// apply **added**. A name already visible to a child — an `exports` entry, or an
/// allowed row of the inherited `environ` — keeps its new value; a fresh name
/// never becomes an export. A `None` save (no prefix, the common `>f`) is a no-op.
pub fn drop_new_exports(
    save: Option<EnvSave>,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let Some(save) = save else {
        return Ok(());
    };
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    for (name, _, old_export) in save.entries {
        let already_visible = old_export.is_some() || was_visible(&state, &name);
        if !already_visible {
            let _ = state.exports.remove(&name);
        }
    }
    Ok(())
}

/// Whether a child already saw `name` **before** the apply: the inherited
/// environment carries a row for it and the env filter allows it — the same
/// predicate `exec/environ.rs::export_entry` applies, so the child-env view and
/// this decision cannot drift (bash imports an inherited row as an export).
fn was_visible(state: &ShellState, name: &ShortCStr) -> bool {
    let carried = state.environ.iter().any(|(k, _)| k == name);
    carried && state.env_filter.is_allowed(name)
}

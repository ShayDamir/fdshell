//! The saved half of `apply`: the previous values collected on first touch,
//! put back in first-touch order. `record` stays beside the struct it fills —
//! its doc comment carries the field-sync rule (tasks #388/#593).

// mutants: -C --lib -- -E 'test(/^run_env::/)'

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ShortCStr};

/// The previous values of the first-touched names, plus the old `IFS` when
/// the prefix sets one: `set_var` syncs `state.ifs`, and the split set is a
/// **field**, so the per-name loop below cannot reach it and a scoped IFS
/// would leak into the parent on restore. (The loop does cover the `IFS`
/// *name*, whose seed the startup put in `strings` — task #388.) The `OPTIND`
/// counter needs its own saved value for the same field reason: the loop syncs
/// the field only when an entry existed (it restores through `set_var`), while
/// the `None` branch removes the name **without** the setter — and bash reaches
/// exactly that branch after `unset OPTIND` (task #593).
pub struct EnvSave {
    pub(crate) entries: Vec<(ShortCStr, Option<ImportedStr>, Option<ImportedStr>)>,
    pub(crate) old_ifs: Option<ShortCStr>,
    pub(crate) old_optind: Option<u64>,
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
    if let Some(optind) = save.old_optind {
        state.optind = optind;
    }
    Ok(())
}

/// Record the previous values of `name` on first touch only, so
/// `FOO=1 FOO=2 cmd` restores the original `FOO`.
pub(super) fn record(save: &mut EnvSave, state: &mut ShellState, name: &ShortCStr) {
    if save.entries.iter().any(|(n, _, _)| n == name) {
        return;
    }
    let old_strings = state.strings.get(name).cloned();
    let old_exports = state.exports.get(name).cloned();
    if name.eq_bytes(b"IFS") {
        save.old_ifs = Some(state.ifs.clone());
    }
    if name.eq_bytes(b"OPTIND") {
        save.old_optind = Some(state.optind);
    }
    save.entries.push((name.clone(), old_strings, old_exports));
}

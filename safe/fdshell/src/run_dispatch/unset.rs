//! `unset`'s run half (task #290): every operand of one statement is applied
//! in the order it was written. The fd arms live here, next to the other
//! descriptor bookkeeping; a shell variable's whole footprint is one
//! `ShellState::unset_var` call, because the state owns those stores.

use crate::error::cmd::CmdError;
use crate::parse::UnsetTarget;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Apply the operand list, then record `$_` and the exit status: bash's `$_`
/// is the last operand's name (the word as written for a shell variable, `%`
/// stripped for an fd one, the array name for `%arr[%src]`), and with no
/// operands it falls back to the command word itself (measured, task #290).
pub(super) fn run(
    targets: &[UnsetTarget],
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    for target in targets {
        unset_one(&mut state, target);
    }
    let last = targets.last().map_or_else(|| c"unset".into(), stored_name);
    state.set_last_arg(last);
    state.set_last_exit(0);
    Ok(())
}

/// One operand, inside its own namespace. `%name` closes the descriptor, drops
/// an fd array of that name, and detaches a named task only — a plain-`&` job
/// is unnamed and stays `wait`'s to reap (task #326). A plain name is a shell
/// variable and touches no fd, which is the `%` = fd namespace rule (STYLE §5).
fn unset_one(state: &mut ShellState, target: &UnsetTarget) {
    match target {
        UnsetTarget::Fd(name) => {
            state.fds.remove(name);
            state.arrays.remove(name);
            state.jobs.remove_named(name);
        }
        UnsetTarget::FdArrayEntry { var, source } => state.remove_array_entry(var, source),
        UnsetTarget::Var(name) => state.unset_var(name),
    }
}

/// The name `unset` leaves in `$_` for one operand.
fn stored_name(target: &UnsetTarget) -> ShortCStr {
    match target {
        UnsetTarget::Fd(name) | UnsetTarget::Var(name) => name.clone(),
        UnsetTarget::FdArrayEntry { var, .. } => var.clone(),
    }
}

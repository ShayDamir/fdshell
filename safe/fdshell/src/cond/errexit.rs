use crate::error::cmd::CmdError;
use crate::options;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// Whether errexit stops the shell now: the option is on, the last command of
/// the list failed, and the shell is not inside an AND/OR list part bash gates
/// ([`gated`], [`crate::state::ShellState::errexit_gated`]). The failing status
/// stays in `last_status`, so the shell's exit code is the failing command's
/// code, as in bash.
pub(crate) fn should_exit(cell: &ForkCell<ShellState>) -> Result<bool, Report<CmdError>> {
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(state.options & options::ERREXIT != 0
        && !state.errexit_gated
        && state.last_status.exit_code() != 0)
}

/// Run `f` as a part of an AND/OR list that bash exempts from errexit:
/// `state.errexit_gated` is set before `f` and restored to the **saved** value
/// before `f`'s result is returned, on every exit path (the pairing rule of
/// [`crate::nest::deeper`], the shape of [`crate::loop_control::in_loop`]).
/// Restoring the saved value, not clearing the flag, is what makes a gated part
/// nested in a gated part restore correctly.
///
/// bash's clause (POSIX 2.9.1, the manual's "Set `-e`"): the shell does not exit
/// for a command that is "part of any command executed in a `&&` or `||` list
/// except the command following the final `&&` or `||`", and the exemption
/// covers **everything executed inside** a compound in that position — a
/// `for`/`while`/`until` body, a `{ …; }` group, an `if` then/elif/else body, a
/// `case` arm, a function-call frame — because each of them reaches
/// [`should_exit`] through its own nested [`crate::cond::run_cond_list`]. The
/// flag rides the state, so it propagates through nesting, `source`d files,
/// function frames and forked children, exactly as bash's execution context
/// does.
pub(crate) fn gated<T, F>(cell: &ForkCell<ShellState>, f: F) -> Result<T, Report<CmdError>>
where
    F: FnOnce() -> Result<T, Report<CmdError>>,
{
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let saved = state.errexit_gated;
    state.errexit_gated = true;
    drop(state); // never hold the borrow across the recursive body
    let result = f();
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.errexit_gated = saved;
    result
}

//! The dispatch arms that only change shell state: the `%fd` assignment and the
//! function definition. The dispatcher in `run_dispatch.rs` keeps the routing and
//! the bare-assignment bookkeeping its sibling modules call.

// mutants: -C --lib -C --test -C umask -C --test -C last_arg -C --test -C arrays -C --test -C unset -C --test -C loops -C --test -C function_keyword -C --test -C cmd_subst -- -E 'test(/^tests::run::/) | binary(umask) | binary(last_arg) | binary(arrays) | binary(unset) | binary(loops) | binary(function_keyword) | binary(cmd_subst)'

use crate::error::cmd::CmdError;
use crate::parse::function_block::FunctionDef;
use crate::state::{FdVar, ShellState};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::{ScriptText, Trace};

/// `%name=%value`: clone the source fd under the destination name, traced to the
/// source's origin, and leave the assignment's own state behind.
pub(super) fn assign_fd(
    var: &ShortCStr,
    value: &ShortCStr,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let src = state.fds.get(value).ok_or(CmdError::FdNotSet)?;
    let fd = src.fd.try_clone().change_context(CmdError::Fd)?;
    let trace = Trace::at(text.start, src.trace.origin.clone());
    state.set_fd_var(var.clone(), FdVar { fd, trace });
    state.clear_last_arg();
    state.set_last_exit(0);
    Ok(())
}

/// A function definition stores its body under the definer's name; the run
/// succeeded, so it leaves `$?` = 0.
pub(super) fn function_def(
    def: &FunctionDef,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state
        .functions
        .insert(def.name.clone(), def.body.data.clone());
    state.set_last_exit(0);
    Ok(())
}

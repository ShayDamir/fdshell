//! `$_` bookkeeping for intercepted commands and `eval`/`source` frames.

use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;

/// Bash sets `$_` to the last argument (or the command name) of an
/// intercepted command. `set --` stores its own expanded value, so skip it.
pub(super) fn set_intercepted_last_arg(
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    if cmdline.pidvar.is_some() {
        return Ok(());
    }
    let is_set_positional =
        cmdline.args.first().is_some_and(|a| a.eq_bytes(b"--")) && cmdline.command.eq_bytes(b"set");
    if is_set_positional {
        return Ok(());
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_arg(
        cmdline
            .args
            .last()
            .cloned()
            .unwrap_or_else(|| cmdline.command.clone()),
    );
    Ok(())
}

/// Run `f` inside an `eval`/`source` frame: inner commands must not update
/// `$_` (bash keeps the `eval`/`source` command's own last argument).
pub(super) fn with_eval_frame<T, F>(
    cell: &ForkCell<ShellState>,
    f: F,
) -> Result<T, Report<CmdError>>
where
    F: FnOnce() -> Result<T, Report<CmdError>>,
{
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.begin_eval();
    }
    let result = f();
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.end_eval();
    result
}

//! `$_` bookkeeping for intercepted commands and `eval`/`source` frames.

// mutants: -C --lib -C --test -C last_arg -- -E 'test(/^intercept::last_arg_frame::/) | binary(last_arg)'

use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;

/// Bash sets `$_` to the last argument (or the command name) of an
/// intercepted command — the last field the command **consumed**, so a handler
/// that ran the substitution pipeline hands its expanded list to `expanded`.
/// The rule itself is [`crate::last_arg::last_word`]; this frame is its
/// dispatch adapter. `set --` decides its own form and stores the value, so
/// skip it here.
pub(super) fn set_intercepted_last_arg(
    cmdline: &CommandLine,
    expanded: Option<&[ShortCStr]>,
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
    let words = expanded.unwrap_or(&cmdline.args);
    state.set_last_arg(crate::last_arg::last_word(words, &cmdline.command));
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

//! `$_` bookkeeping for intercepted commands and `eval`/`source` frames.

use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;

/// Bash sets `$_` to the last argument (or the command name) of an
/// intercepted command — the last field the command **consumed**, so a handler
/// that ran the substitution pipeline hands its expanded list to `expanded`.
/// A fully quoted word that expands to nothing is an (empty) field; the command
/// word is the fallback only when there were no operand words at all.
/// `set --` stores its own expanded value, so skip it.
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
    state.set_last_arg(last_field(cmdline, expanded));
    Ok(())
}

/// The last word the command consumed: the field list the handler reports, or
/// the parsed words when it reports none; the command word when there were no
/// operand words.
fn last_field(cmdline: &CommandLine, expanded: Option<&[ShortCStr]>) -> ShortCStr {
    let words: &[ShortCStr] = expanded.unwrap_or(&cmdline.args);
    words
        .last()
        .cloned()
        .unwrap_or_else(|| cmdline.command.clone())
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

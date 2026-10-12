mod complete;
mod control;
mod line;
mod run;
mod trailing;

pub(crate) use crate::cond::run_cond_list;
pub(crate) use crate::script::run_script;
pub use run::run;

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Run a whole input as the interactive REPL does: a `LoopControl` left over
/// by the whole script is a control signal that outlived its scope.
pub(super) fn handle(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    if let Some(control) = run_script(text, cell)? {
        control::out_of_scope(control)?;
    }
    Ok(())
}

/// Run one `-c`/script-file input and report the shell's exit status.
pub fn exec_cmd(text: &ScriptText, cell: &ForkCell<ShellState>) -> Result<i32, Report<CmdError>> {
    if let Some(control) = run_script(text, cell)? {
        control::out_of_scope(control)?;
    }
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(state.last_status.exit_code())
}

#[cfg(test)]
mod tests;

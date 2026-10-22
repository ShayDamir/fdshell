//! The pipeline path: expand every stage's word 0, then launch the whole
//! pipeline (a pipeline is one launch, so its stages expand together).

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::Pipeline;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

pub(crate) fn run_pipeline(
    pipeline: &Pipeline,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // Word 0 first, with the shell's own `IFS`: a stage's scoped `IFS` applies
    // in its child, after its args were split (as in the simple-command path).
    let stages = crate::command_word::expand_pipeline(pipeline, cell)?;
    let envs = crate::run_env::expand_pipeline(&stages, text, cell)?;
    let status =
        crate::postlaunch::run_pipeline(&stages, cell, &envs).change_context(CmdError::Pipeline)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.last_status = status;
    Ok(None)
}

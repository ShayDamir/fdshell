use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

pub(crate) fn run_one(
    line: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let parsed = crate::parse::parse(line).change_context(CmdError::Parse)?;
    match &parsed {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if crate::intercept::try_intercept(line, cmdline, cell)? {
                return Ok(None);
            }
            let outcome = crate::launch::launch(cell, cmdline).change_context(CmdError::Launch)?;
            {
                let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
                state.last_status =
                    crate::postlaunch::finish_cmd(cmdline.clone(), outcome, &mut state)
                        .change_context(CmdError::Launch)?;
            }
            Ok(None)
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            let status = crate::postlaunch::run_pipeline(pipeline.clone(), cell)
                .change_context(CmdError::Pipeline)?;
            let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
            state.last_status = status;
            Ok(None)
        }
        crate::parse::ParsedLine::For(forblock) => {
            crate::for_run::run_for(forblock, cell)?;
            Ok(None)
        }
        crate::parse::ParsedLine::While(whileblock) => {
            crate::loop_::run_loop(&whileblock.condition, &whileblock.body, true, cell)?;
            Ok(None)
        }
        crate::parse::ParsedLine::Until(untilblock) => {
            crate::loop_::run_loop(&untilblock.condition, &untilblock.body, false, cell)?;
            Ok(None)
        }
        crate::parse::ParsedLine::Case(caseblock) => crate::case_exec::run_case(caseblock, cell),
        crate::parse::ParsedLine::If(ifblock) => crate::if_exec::run_if(ifblock, cell),
        _ => crate::run_dispatch::run_simple(&parsed, cell),
    }
}

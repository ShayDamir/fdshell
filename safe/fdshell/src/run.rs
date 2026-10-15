use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::ParsedLine;
use crate::state::ShellState;

mod cmd;
mod parent;
mod pipeline;

pub(crate) fn run_one(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let text = crate::alias_expand::expand_alias(text, cell)?;
    let text = crate::brace_expand::expand(&text)?;
    let parsed = crate::parse::parse(&text).change_context(CmdError::Parse)?;
    run_parsed(&parsed, &text, cell)
}

/// Dispatch an already-parsed statement. Split out of [`run_one`] so the
/// `time` hook can run its inner statement through the identical path.
pub(crate) fn run_parsed(
    parsed: &ParsedLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    match parsed {
        ParsedLine::Cmd(cmdline) => cmd::run_cmd(cmdline, text, cell),
        ParsedLine::Pipeline(pipeline) => pipeline::run_pipeline(pipeline, text, cell),
        ParsedLine::For(forblock) => {
            if let Some(control) = crate::for_run::run_for(forblock, text, cell)? {
                return Ok(Some(control));
            }
            Ok(None)
        }
        ParsedLine::While(whileblock) => {
            if let Some(control) = crate::loop_::run_loop(whileblock, true, cell)? {
                return Ok(Some(control));
            }
            Ok(None)
        }
        ParsedLine::Until(untilblock) => {
            if let Some(control) = crate::loop_::run_loop(untilblock, false, cell)? {
                return Ok(Some(control));
            }
            Ok(None)
        }
        ParsedLine::Case(caseblock) => crate::case_exec::run_case(caseblock, cell),
        ParsedLine::If(ifblock) => crate::if_exec::run_if(ifblock, cell),
        ParsedLine::Group(group) => crate::group_exec::run_group(group, cell),
        ParsedLine::Wait(waitblock) => crate::wait::run_wait(waitblock, cell),
        ParsedLine::Time(block) => crate::time_exec::run_time(block, cell),
        _ => crate::run_dispatch::run_simple(parsed, text, cell),
    }
}

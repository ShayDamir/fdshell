// mutants: -C --lib -C --test -C trap -C --test -C pipeline -C --test -C loops -C --test -C time -- -E 'test(/^tests::run::/) | binary(trap) | binary(pipeline) | binary(loops) | binary(time)'

use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::ParsedLine;
use crate::state::ShellState;

mod async_job;
mod cmd;
mod parent;
mod pipeline;

/// Run pending traps at the boundary of a completed simple command or
/// pipeline, then propagate the command's own result. The action runs
/// **before** a failing command's report is printed (bash measured, task
/// #585: a signal landing during a blocking `read` shows the action, then the
/// report — the plan's pin), and control flows merge with the action's
/// `break`/`continue` winning over the command's own (the plan's rule).
fn run_at_boundary(
    result: Result<Option<LoopControl>, Report<CmdError>>,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    match result {
        Ok(control) => crate::trap_exec::run_pending(cell).map(|raised| raised.or(control)),
        Err(report) => {
            crate::trap_exec::run_pending(cell)?;
            Err(report)
        }
    }
}

pub(crate) fn run_one(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
    background: bool,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let text = crate::alias_expand::expand_alias(text, cell)?;
    let text = crate::brace_expand::expand(&text)?;
    let parsed = crate::parse::parse(&text).change_context(CmdError::Parse)?;
    run_parsed(&parsed, &text, cell, background)
}

/// Dispatch an already-parsed statement. Split out of [`run_one`] so the
/// `time` hook can run its inner statement through the identical path.
/// `background` is the plain-`&` flag: a `Cmd`/`Pipeline` or a loop-control
/// keyword with it becomes an async job (see [`async_job`], whose child runs the
/// keyword and lets its signal die with the child — task #362); every other arm
/// runs foreground (measured divergence, task #326 — compound background is a
/// filed follow-up).
pub(crate) fn run_parsed(
    parsed: &ParsedLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
    background: bool,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    match parsed {
        ParsedLine::Cmd(cmdline) if background => async_job::spawn(parsed, text, cell),
        ParsedLine::Pipeline(pipeline) if background => async_job::spawn(parsed, text, cell),
        // A loop-control keyword is a job too: its signal dies with the child
        // (task #362), which is what keeps the parent's loop running.
        ParsedLine::Break(_) | ParsedLine::Continue(_) | ParsedLine::Return(_) if background => {
            async_job::spawn(parsed, text, cell)
        }
        ParsedLine::Cmd(cmdline) => run_at_boundary(cmd::run_cmd(cmdline, text, cell), cell),
        ParsedLine::Pipeline(pipeline) => {
            run_at_boundary(pipeline::run_pipeline(pipeline, text, cell), cell)
        }
        ParsedLine::For(forblock) => {
            if let Some(control) = crate::for_run::run_for(forblock, text, cell)? {
                return Ok(Some(control));
            }
            Ok(None)
        }
        // The arithmetic `for`'s control flows out exactly as the word form's.
        ParsedLine::ArithFor(block) => crate::for_run::arith::run(block, cell),
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

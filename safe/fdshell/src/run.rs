use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;

pub(crate) fn run_one(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let text = crate::alias_expand::expand_alias(text, cell)?;
    let text = crate::brace_expand::expand(&text)?;
    let parsed = crate::parse::parse(&text).change_context(CmdError::Parse)?;
    match &parsed {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            // A scoped `NAME=value` prefix: expanded once, applied around the
            // parent-side handlers, and handed to the forked child (which
            // applies it itself after arg substitution).
            let env = crate::run_env::expand(cmdline, &text, cell)?;
            let save = crate::run_env::apply(&env, cell)?;
            let control = match parent_command(&text, cmdline, cell) {
                Ok(control) => {
                    crate::run_env::restore(save, cell)?;
                    control
                }
                Err(e) => {
                    let _ = crate::run_env::restore(save, cell);
                    return Err(e);
                }
            };
            if let Some(control) = control {
                return Ok(control);
            }
            // Forked commands must expand their args against the previous `$_`,
            // so the child (not the parent) reports the new value via the
            // capture socket, consumed by `finish_cmd`.
            let outcome =
                crate::launch::launch(cell, cmdline, &env).change_context(CmdError::Launch)?;
            {
                let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
                state.last_status =
                    crate::postlaunch::finish_cmd(cmdline.clone(), outcome, &mut state)
                        .change_context(CmdError::Launch)?;
            }
            Ok(None)
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            let envs = crate::run_env::expand_pipeline(pipeline, &text, cell)?;
            let status = crate::postlaunch::run_pipeline(pipeline.clone(), cell, &envs)
                .change_context(CmdError::Pipeline)?;
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.last_status = status;
            Ok(None)
        }
        crate::parse::ParsedLine::For(forblock) => {
            if let Some(control) = crate::for_run::run_for(forblock, &text, cell)? {
                return Ok(Some(control));
            }
            Ok(None)
        }
        crate::parse::ParsedLine::While(whileblock) => {
            if let Some(control) = crate::loop_::run_loop(whileblock, true, cell)? {
                return Ok(Some(control));
            }
            Ok(None)
        }
        crate::parse::ParsedLine::Until(untilblock) => {
            if let Some(control) = crate::loop_::run_loop(untilblock, false, cell)? {
                return Ok(Some(control));
            }
            Ok(None)
        }
        crate::parse::ParsedLine::Case(caseblock) => crate::case_exec::run_case(caseblock, cell),
        crate::parse::ParsedLine::If(ifblock) => crate::if_exec::run_if(ifblock, cell),
        crate::parse::ParsedLine::Wait(waitblock) => crate::wait::run_wait(waitblock, cell),
        _ => crate::run_dispatch::run_simple(&parsed, &text, cell),
    }
}

/// The parent-side handlers of a command (a user function shadows builtins
/// and interceptors, as in bash; a `builtin` prefix bypasses the function
/// lookup), run inside the scoped-assignment window. `None` when neither
/// handled the command.
fn parent_command(
    text: &ScriptText,
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Option<LoopControl>>, Report<CmdError>> {
    if let Some(control) = crate::function_call::try_call(text, cmdline, cell)? {
        return Ok(Some(control));
    }
    crate::intercept::try_intercept(text, cmdline, cell)
}

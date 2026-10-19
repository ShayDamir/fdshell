use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::{FdVar, ShellState};
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, Trace};

mod arith_cmd;
mod array_ops;
mod assign_str;
mod loop_ctl;

/// Handle simple state-modifying parsed lines (assign, unset, umask, break, continue).
pub(crate) fn run_simple(
    parsed: &crate::parse::ParsedLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    if arith_cmd::run(parsed, cell)? || array_ops::run(parsed, text, cell)? {
        return Ok(None);
    }
    match parsed {
        crate::parse::ParsedLine::AssignFd { var, value } => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            let src = state.fds.get(value).ok_or(CmdError::FdNotSet)?;
            let fd = src.fd.try_clone().change_context(CmdError::Fd)?;
            let trace = Trace::at(text.start, src.trace.origin.clone());
            state.set_fd_var(var.clone(), FdVar { fd, trace });
            state.clear_last_arg();
            state.set_last_exit(0);
        }
        crate::parse::ParsedLine::AssignStr(assign) => {
            assign_str::set(assign, text, cell)?;
            finish_assignment(cell)?;
        }
        crate::parse::ParsedLine::AssignStrs(assigns) => {
            for assign in assigns {
                assign_str::set(assign, text, cell)?;
            }
            finish_assignment(cell)?;
        }
        crate::parse::ParsedLine::Unset(var) => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.fds.remove(var);
            state.arrays.remove(var);
            // `unset` detaches a named task only — a plain-`&` job is unnamed
            // and stays `wait`'s to reap (task #326).
            state.jobs.remove_named(var);
            state.set_last_arg(var.clone());
            state.set_last_exit(0);
        }
        crate::parse::ParsedLine::Umask(mask) => {
            if let Some(m) = mask {
                sys::umask::set(*m);
            } else {
                let s = alloc::format!("{:04o}", sys::umask::get());
                sys::OUT.write_all(s.as_bytes()).ok();
            }
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.set_last_exit(0);
        }
        crate::parse::ParsedLine::Function(def) => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state
                .functions
                .insert(def.name.clone(), def.body.data.clone());
            state.set_last_exit(0);
        }
        crate::parse::ParsedLine::Break(n) => return loop_ctl::raise_break(n, cell),
        crate::parse::ParsedLine::Continue(n) => return loop_ctl::raise_continue(n, cell),
        crate::parse::ParsedLine::Return(code) => {
            if let Some(n) = code {
                let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
                state.set_last_exit(*n);
            }
            return Ok(Some(LoopControl::Return));
        }
        _ => {}
    }
    Ok(None)
}

/// The state a bare assignment statement leaves behind: `$_` cleared (as
/// bash does) and the exit status zeroed.
fn finish_assignment(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.clear_last_arg();
    state.set_last_exit(0);
    Ok(())
}

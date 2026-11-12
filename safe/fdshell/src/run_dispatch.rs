// mutants: -C --lib -C --test -C umask -C --test -C last_arg -C --test -C arrays -C --test -C unset -C --test -C loops -C --test -C function_keyword -C --test -C cmd_subst -- -E 'test(/^tests::run::/) | binary(umask) | binary(last_arg) | binary(arrays) | binary(unset) | binary(loops) | binary(function_keyword) | binary(cmd_subst)'
use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::{FdVar, ShellState};
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, Trace};

mod arith_cmd;
mod array_ops;
mod assign_array;
mod assign_str;
mod loop_ctl;
mod unset;

/// Handle simple state-modifying parsed lines (assign, unset, umask, break, continue).
pub(crate) fn run_simple(
    parsed: &crate::parse::ParsedLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // The per-statement twin of `command_word::expand`'s scratch reset: an
    // assignment statement never reaches that pass, so the slot it reads must be
    // the one its **own** expansion filled — 0 when it performed none (task #533,
    // bash's `x=$(false); y=1; echo rc=$?` → `rc=0`). An argument substitution runs
    // in the child that launches the command, so only a parent-side one can leak.
    cell.borrow_mut()
        .change_context(CmdError::Never)?
        .last_substitution = None;

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
        crate::parse::ParsedLine::AssignArray(assigns) => {
            assign_array::run(assigns, text, cell)?;
        }
        crate::parse::ParsedLine::Unset(targets) => unset::run(targets, cell)?,
        crate::parse::ParsedLine::Umask { mask, dash_p } => {
            if let Some(m) = mask {
                sys::umask::set(*m);
            } else if *dash_p {
                let s = alloc::format!("umask {:04o}\n", sys::umask::get());
                sys::OUT.write_all(s.as_bytes()).ok();
            } else {
                let s = alloc::format!("{:04o}\n", sys::umask::get());
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
/// bash does) and the exit status set to the last substitution the statement
/// performed — 0 when it performed none (POSIX #2.9.1, task #533). The `$_`
/// half is unchanged.
fn finish_assignment(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let code = state
        .last_substitution
        .as_ref()
        .map_or(0, |s| s.exit_code());
    state.clear_last_arg();
    state.set_last_exit(code);
    Ok(())
}

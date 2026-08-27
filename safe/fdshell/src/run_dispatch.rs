use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::{FdVar, ShellState};
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, Trace};

mod array_ops;

/// Handle simple state-modifying parsed lines (assign, unset, umask, break, continue).
pub(crate) fn run_simple(
    parsed: &crate::parse::ParsedLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    if array_ops::run(parsed, text, cell)? {
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
        crate::parse::ParsedLine::AssignStr { var, value } => {
            let (expanded, _) =
                crate::substitute::substitute_arg(value, &[], &mut HashMap::new(), cell)
                    .change_context(CmdError::Resolve)?;
            let origin = crate::run_origin::assign_origin(value, text.origin.clone(), cell)?;
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.set_var(
                var.clone(),
                ImportedStr::new(expanded, Trace::at(text.start, origin)),
            );
            // Bash clears `$_` for plain assignments.
            state.clear_last_arg();
            state.set_last_exit(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.fds.remove(var);
            state.arrays.remove(var);
            state.tasks.remove(var);
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
        crate::parse::ParsedLine::Break => return Ok(Some(LoopControl::Break)),
        crate::parse::ParsedLine::Continue => return Ok(Some(LoopControl::Continue)),
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

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Handle simple state-modifying parsed lines (assign, unset, umask, break, continue).
pub(crate) fn run_simple(
    parsed: &crate::parse::ParsedLine,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    match parsed {
        crate::parse::ParsedLine::AssignFd { var, value } => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            let src = state
                .fds
                .get(value)
                .ok_or(CmdError::FdNotSet)?
                .try_clone()
                .change_context(CmdError::Fd)?;
            state.fds.insert(var.clone(), src);
            state.set_last_exit(0);
        }
        crate::parse::ParsedLine::AssignStr { var, value } => {
            let expanded = {
                crate::substitute::substitute_arg(value, &mut HashMap::new(), cell)
                    .change_context(CmdError::Resolve)?
            };
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.strings.insert(
                var.clone(),
                ShortCStr::from_vec(expanded.into_bytes()).change_context(CmdError::Resolve)?,
            );
            state.set_last_exit(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.fds.remove(var);
            state.tasks.remove(var);
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
        crate::parse::ParsedLine::Break => return Ok(Some(LoopControl::Break)),
        crate::parse::ParsedLine::Continue => return Ok(Some(LoopControl::Continue)),
        _ => {}
    }
    Ok(None)
}

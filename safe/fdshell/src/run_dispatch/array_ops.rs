use crate::error::cmd::CmdError;
use crate::parse::ParsedLine;
use crate::state::{FdVar, ShellState};
use error_stack::{Report, ResultExt, bail};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, Trace};

/// Run an fd-array statement (indexed read-out, empty init, append, entry unset).
/// Returns `true` when `parsed` was an array statement.
pub(super) fn run(
    parsed: &ParsedLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    match parsed {
        ParsedLine::AssignFdIndex { var, value, index } => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            let entry = match state.arrays.get(value) {
                Some(arr) => arr.get(*index).ok_or(CmdError::ArrayIndexOutOfRange {
                    name: value.clone(),
                    index: *index,
                })?,
                None if state.fds.contains_key(value) => {
                    bail!(CmdError::NotAnArray {
                        name: value.clone()
                    })
                }
                None => bail!(CmdError::FdNotSet),
            };
            let fd = entry.fd.try_clone().change_context(CmdError::Fd)?;
            let trace = Trace::at(text.start, entry.trace.origin.clone());
            state.set_fd_var(var.clone(), FdVar { fd, trace });
            state.clear_last_arg();
            state.set_last_exit(0);
            Ok(true)
        }
        ParsedLine::AssignArrayEmpty { var } => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.set_empty_array(var.clone());
            state.clear_last_arg();
            state.set_last_exit(0);
            Ok(true)
        }
        ParsedLine::AppendFd { var, value } => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            if state.arrays.get(var).is_none() && state.fds.contains_key(var) {
                bail!(CmdError::NotAnArray { name: var.clone() });
            }
            let src = state.fds.get(value).ok_or(CmdError::FdNotSet)?;
            let fd = src.fd.try_clone().change_context(CmdError::Fd)?;
            let trace = Trace::at(text.start, src.trace.origin.clone());
            state.append_array_entry(var, fd, value, trace);
            state.clear_last_arg();
            state.set_last_exit(0);
            Ok(true)
        }
        ParsedLine::UnsetArrayEntry { var, source } => {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.remove_array_entry(var, source);
            state.set_last_arg(var.clone());
            state.set_last_exit(0);
            Ok(true)
        }
        _ => Ok(false),
    }
}

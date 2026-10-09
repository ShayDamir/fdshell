//! `local` with no arguments: the call's locals as `local NAME[=value]` lines,
//! sorted by name. A declared-unset local prints as `local NAME` (the POSIX/dash
//! form; bash prints `declare -- NAME="value"`, an accepted divergence).

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

pub(super) fn list_locals(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let out = {
        // A shared borrow, scoped so it is dropped before the `set_last_exit` borrow.
        let state = cell.borrow().change_context(CmdError::Never)?;
        let mut names = state.local_names();
        crate::intercept::set_list::sort_by_bytes(&mut names);
        let mut out = Vec::new();
        for name in names {
            out.extend_from_slice(b"local ");
            out.extend_from_slice(name.as_bytes().change_context(CmdError::Resolve)?);
            if let Some(value) = state.strings.get(name) {
                out.push(b'=');
                out.extend_from_slice(value.value.as_bytes().change_context(CmdError::Resolve)?);
            }
            out.push(b'\n');
        }
        out
    };
    sys::OUT.write_all(&out).ok();
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(())
}

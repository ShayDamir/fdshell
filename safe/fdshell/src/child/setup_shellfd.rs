//! The forked child's capture-socket setup, extracted from `child/run.rs`
//! (task #481): the child runs with its own copy of the state, so the socket the
//! parent handed it becomes the state's `shell_sock` and flips the capture
//! switch for the whole child. A stage with no capture socket turns the switch
//! off, exactly as the plain launch's child does.

use crate::error::child_process::ChildProcessError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

pub(super) fn setup_shellfd(
    sock: Option<&sys::LocalFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<ChildProcessError>> {
    if let Some(s) = sock {
        let mut state = cell
            .borrow_mut()
            .change_context(ChildProcessError::BorrowFailed)?;
        state.shell_sock = Some(
            s.try_clone()
                .change_context(ChildProcessError::ExportFailed)?,
        );
        sys::shellfd::set_capture_active(true);
    } else {
        sys::shellfd::set_capture_active(false);
    }
    Ok(())
}

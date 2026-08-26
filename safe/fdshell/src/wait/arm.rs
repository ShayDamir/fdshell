mod reap;

use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

use crate::error::wait::WaitError;
use crate::parse::wait_block::WaitArm;
use crate::state::ShellState;

use super::round::ReleaseKey;

/// Fork one arm child for a ready descriptor and run its body. On reap, drain
/// the arm's captures and apply the keep/release protocol. Returns the arm's
/// exit status. In the forked child this function does not return (it exits).
pub(crate) fn dispatch(
    arm: &WaitArm,
    raw: Option<i32>,
    release: &ReleaseKey,
    finished: bool,
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<WaitError>> {
    if let ReleaseKey::Task(name) = release {
        reap_task(name, cell)?;
    }
    let (parent_end, child_end) =
        sys::net::socketpair_with_passcred().change_context(WaitError::ArmFork)?;
    let (child_pid, pidfd_opt) =
        sys::fork_pidfd::fork_pidfd_cell(cell).change_context(WaitError::ArmFork)?;
    match pidfd_opt {
        None => {
            let code = reap::child_main(arm, raw, child_end, cell);
            match code {
                Ok(c) => sys::exit(c),
                Err(report) => {
                    let _ = writeln!(crate::io::Stderr, "{report:?}");
                    sys::exit(1);
                }
            }
        }
        Some(pidfd) => {
            drop(child_end);
            reap::parent_reap(arm, release, finished, parent_end, child_pid, pidfd, cell)
        }
    }
}

/// Reap a background task at fire time, harvesting its captures and preloading
/// `$?` so the arm body reads the child's status bash-style.
fn reap_task(name: &ShortCStr, cell: &ForkCell<ShellState>) -> Result<(), Report<WaitError>> {
    let mut s = cell.borrow_mut().change_context(WaitError::Never)?;
    let task = s
        .tasks
        .remove(name)
        .ok_or(WaitError::TaskNotFound { name: name.clone() })?;
    let status = sys::wait_pidfd::wait_pidfd(&task.pidfd).change_context(WaitError::Reap)?;
    if matches!(status, WaitStatus::Exited(0))
        && let Some(capture_fd) = task.capture_fd
    {
        crate::capture::capture_and_commit(capture_fd, task.child_pid, task.captures, &mut s)
            .change_context(WaitError::ArmCapture)?;
    }
    s.last_status = status;
    Ok(())
}

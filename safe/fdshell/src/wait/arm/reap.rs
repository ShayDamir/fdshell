use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{LocalFd, Origin, Trace};

use crate::error::wait::WaitError;
use crate::parse::wait_block::WaitArm;
use crate::state::{FdVar, ShellState};

use crate::wait::round::ReleaseKey;

/// The arm child: bind the matched fd to `?`, point the capture socket at the
/// parent, run the arm body, and exit with its status.
pub(super) fn child_main(
    arm: &WaitArm,
    raw: Option<i32>,
    child_end: LocalFd,
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<WaitError>> {
    if let Some(raw) = raw {
        let fd = sys::dup::dup_cloexec(raw).change_context(WaitError::ArmFork)?;
        let mut s = cell.borrow_mut().change_context(WaitError::Never)?;
        s.fds.insert(
            c"?".into(),
            FdVar {
                fd,
                trace: Trace::boundary(Origin::Shell),
            },
        );
    }
    {
        let mut s = cell.borrow_mut().change_context(WaitError::Never)?;
        s.set_shell_sock(child_end);
    }
    sys::shellfd::set_capture_active(true);
    if let Err(report) = crate::repl::run_script(&arm.body, cell) {
        let _ = writeln!(crate::io::Stderr, "{report:?}");
    }
    let code = cell
        .borrow()
        .change_context(WaitError::Never)?
        .last_status
        .exit_code();
    Ok(code)
}

/// The parent: reap the arm, drain its captures, and keep or release the fd.
pub(super) fn parent_reap(
    arm: &WaitArm,
    release: &ReleaseKey,
    finished: bool,
    parent_end: LocalFd,
    child_pid: sys::Pid,
    pidfd: LocalFd,
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<WaitError>> {
    let status = sys::wait_pidfd::wait_pidfd(&pidfd).change_context(WaitError::Reap)?;
    let captures = arm.captures.clone();
    if !captures.is_empty() {
        let mut s = cell.borrow_mut().change_context(WaitError::Never)?;
        crate::capture::capture_and_commit(parent_end, child_pid, captures, &mut s)
            .change_context(WaitError::ArmCapture)?;
    }
    let keep = status.exit_code() == 0 && !finished;
    if !keep {
        let mut s = cell.borrow_mut().change_context(WaitError::Never)?;
        apply_release(release, &mut s);
    }
    Ok(status.exit_code())
}

/// Close the descriptor an arm chose to release, per the keep/release protocol.
fn apply_release(key: &ReleaseKey, s: &mut ShellState) {
    match key {
        ReleaseKey::None => {}
        ReleaseKey::Var(name) => {
            s.fds.remove(name);
            s.arrays.remove(name);
        }
        ReleaseKey::Array { arr, source } => {
            s.remove_array_entry(arr, source);
        }
        ReleaseKey::Task(_) => {}
    }
}

use crate::error::task::TaskError;
use crate::state::ShellState;
use crate::task::Task;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

/// Block on `task`'s pidfd and commit its captures on a clean exit.
pub(super) fn reap(task: Task, state: &mut ShellState) -> Result<WaitStatus, Report<TaskError>> {
    let (status, times) = task
        .pidfd
        .wait_pidfd_rusage()
        .change_context(TaskError::Wait)?;
    state.add_child_times(times);
    if let WaitStatus::Exited(0) = status
        && let Some(capture_fd) = task.capture_fd
    {
        crate::capture::capture_and_commit(capture_fd, task.child_pid, task.captures, state)
            .change_context(TaskError::Capture)?;
    }
    Ok(status)
}

/// Reap every background task; the last reaped status wins.
pub(super) fn wait_all(state: &mut ShellState) -> Result<WaitStatus, Report<TaskError>> {
    let mut last = WaitStatus::Exited(0);
    let keys: Vec<ShortCStr> = state.tasks.keys().cloned().collect();
    for key in keys {
        let Some(task) = state.tasks.remove(&key) else {
            continue;
        };
        last = reap(task, state)?;
    }
    Ok(last)
}

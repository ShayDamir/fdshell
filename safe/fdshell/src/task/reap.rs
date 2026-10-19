use crate::error::task::TaskError;
use crate::state::ShellState;
use crate::task::Task;
use error_stack::{Report, ResultExt};
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

/// Reap every background job: the named table, then the plain-`&` deque in
/// FIFO order. POSIX: `wait` with no operands returns 0 (measured against
/// bash), so the reaped statuses feed only the accumulated child times.
pub(super) fn wait_all(state: &mut ShellState) -> Result<WaitStatus, Report<TaskError>> {
    for key in state.jobs.named_keys() {
        let Some(task) = state.jobs.remove_named(&key) else {
            continue;
        };
        reap(task, state)?;
    }
    for task in state.jobs.drain_anon() {
        reap(task, state)?;
    }
    Ok(WaitStatus::Exited(0))
}

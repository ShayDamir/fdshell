use crate::error::pipeline::PipelineError;
use error_stack::{Report, ResultExt};
use sys::siginfo::WaitStatus;

/// Reap every pipeline child: the last child's status is the pipeline's
/// result; the others' CPU times are accumulated for `times`.
pub(crate) fn reap_children(
    children: &[(sys::Pid, sys::LocalFd)],
) -> Result<(WaitStatus, sys::getrusage::CpuTimes), Report<PipelineError>> {
    let last = children.last().ok_or(PipelineError::Pipeline)?;
    let (status, mut times) = last
        .1
        .wait_pidfd_rusage()
        .change_context(PipelineError::Pipeline)?;
    for ch in children.iter().take(children.len().saturating_sub(1)) {
        if let Ok((_, t)) = ch.1.wait_pidfd_rusage() {
            times.utime = times.utime.saturating_add(t.utime);
            times.stime = times.stime.saturating_add(t.stime);
        }
    }
    Ok((status, times))
}

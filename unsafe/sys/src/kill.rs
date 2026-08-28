use crate::Pid;

/// Send signal `sig` to process `pid`.
pub fn kill(pid: Pid, sig: i32) -> Result<(), crate::SyscallError> {
    // SAFETY: `pid` is a valid pid; `sig` is a signal number; an unknown pid
    // returns -1/`ESRCH`, caught by `cvt`.
    crate::cvt(unsafe { libc::kill(pid.as_raw(), sig) as isize })?;
    Ok(())
}

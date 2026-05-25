use crate::siginfo::{SigInfo, WaitStatus};
use crate::{Fd, cvt};

pub fn wait_pidfd(pidfd: &Fd) -> Result<WaitStatus, i32> {
    // SAFETY: SigInfo is integer types; zeroed is valid.
    let mut info: SigInfo = unsafe { core::mem::zeroed() };

    // SAFETY: SYS_waitid (247) is valid on x86_64 Linux. pidfd is a valid
    // pidfd fd. info is writable memory of the right size for the kernel.
    cvt(unsafe {
        libc::syscall(
            libc::SYS_waitid,
            libc::P_PIDFD as i64,
            pidfd.as_raw() as i64,
            &raw mut info,
            libc::WEXITED as i64,
            0i64,
        ) as isize
    })?;

    Ok(match info.si_code {
        libc::CLD_EXITED => WaitStatus::Exited(info.si_status),
        libc::CLD_KILLED | libc::CLD_DUMPED => WaitStatus::Signaled(info.si_status),
        _ => return Err(crate::errno::EINVAL),
    })
}

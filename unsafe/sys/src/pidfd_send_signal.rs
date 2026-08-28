use crate::LocalFd;

pub const SIGKILL: i32 = libc::SIGKILL;
pub const SIGTERM: i32 = libc::SIGTERM;

/// Send signal `sig` to the process referenced by `pidfd`.
///
/// A simple signal is sent (`info == NULL`, `flags == 0`), so no `siginfo_t`
/// is constructed. `pidfd` must be a valid pidfd (e.g. from `clone3`).
pub fn send_signal(pidfd: &LocalFd, sig: i32) -> Result<(), crate::SyscallError> {
    // SAFETY: SYS_pidfd_send_signal (424) is valid on Linux ≥5.1 x86_64.
    // `pidfd` is a valid pidfd; `sig` is a signal number; NULL info with
    // flags 0 sends a simple signal, so no extra memory is dereferenced.
    crate::cvt(unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw() as i64,
            sig as i64,
            0usize,
            0u64,
        ) as isize
    })?;
    Ok(())
}

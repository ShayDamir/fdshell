use crate::LocalFd;

pub const EFD_SEMAPHORE: i32 = libc::EFD_SEMAPHORE;
pub const EFD_CLOEXEC: i32 = crate::fcntl::O_CLOEXEC;
pub const EFD_NONBLOCK: i32 = crate::fcntl::O_NONBLOCK;

/// Create an eventfd with the given initial counter and `CLOEXEC` set. The fd
/// becomes readable when the counter is non-zero.
pub fn eventfd(init: u32, flags: i32) -> Result<LocalFd, crate::SyscallError> {
    // SAFETY: `eventfd` returns a new fd on success or -1 on error, checked
    // by `cvt`; `init` is the initial counter value.
    let ret = crate::cvt(unsafe { libc::eventfd(init, flags | EFD_CLOEXEC) as isize })?;
    // SAFETY: `EFD_CLOEXEC` is set, satisfying the `LocalFd` invariant.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}

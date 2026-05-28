use crate::LocalFd;

pub fn pipe2(flags: i32) -> Result<(LocalFd, LocalFd), i32> {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid mutable reference to 2 `i32`s; `pipe2` writes
    // exactly 2 file descriptors into it. Invalid flags return `EINVAL`.
    crate::cvt(unsafe { libc::pipe2(fds.as_mut_ptr(), flags) as isize })?;
    let [rd, wr] = fds;
    // SAFETY: `pipe2` with `O_CLOEXEC` sets CLOEXEC on both ends.
    let rd = unsafe { LocalFd::from_raw(rd) };
    let wr = unsafe { LocalFd::from_raw(wr) };
    Ok((rd, wr))
}

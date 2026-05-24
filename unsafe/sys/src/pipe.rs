use crate::Fd;

pub fn pipe2(flags: i32) -> Result<(Fd, Fd), i32> {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid mutable reference to 2 `i32`s; `pipe2` writes
    // exactly 2 file descriptors into it. Invalid flags return `EINVAL`.
    crate::cvt(unsafe { libc::pipe2(fds.as_mut_ptr(), flags) as isize })?;
    let [rd, wr] = fds;
    Ok((rd, wr))
}

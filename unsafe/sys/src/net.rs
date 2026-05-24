use crate::Fd;

pub fn socketpair() -> Result<(Fd, Fd), i32> {
    let mut pair = [0i32; 2];
    // SAFETY: `pair` is a valid mutable reference to 2 `i32`s; `socketpair` writes
    // exactly 2 fds into it. Invalid input is handled by `cvt`.
    crate::cvt(unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
            0,
            pair.as_mut_ptr(),
        ) as isize
    })?;
    let [a, b] = pair;
    // SAFETY: both fds have CLOEXEC set by `SOCK_CLOEXEC`.
    let a = unsafe { Fd::from_raw(a) };
    let b = unsafe { Fd::from_raw(b) };
    Ok((a, b))
}

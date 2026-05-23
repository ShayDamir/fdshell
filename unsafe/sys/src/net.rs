use crate::Fd;

pub fn socketpair(pair: &mut [Fd; 2]) -> Result<(), i32> {
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
    Ok(())
}

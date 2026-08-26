use crate::cvt;

/// Poll event bits, mirroring the kernel `pollfd.events`.
pub const POLLIN: i16 = 0x0001;
pub const POLLPRI: i16 = 0x0002;
pub const POLLOUT: i16 = 0x0004;
pub const POLLERR: i16 = 0x0008;
pub const POLLHUP: i16 = 0x0010;
pub const POLLNVAL: i16 = 0x0020;
pub const POLLRDHUP: i16 = 0x2000;

/// A descriptor to poll: `fd` + requested `events`; `revents` is filled by [`poll`].
#[repr(C)]
pub struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

impl PollFd {
    pub const fn new(fd: i32, events: i16) -> Self {
        Self {
            fd,
            events,
            revents: 0,
        }
    }
}

/// Poll a set of descriptors for up to `timeout_ms` (`-1` blocks indefinitely).
/// Returns the number of descriptors with non-zero `revents`, or `0` on timeout.
pub fn poll(fds: &mut [PollFd], timeout_ms: i32) -> Result<usize, crate::SyscallError> {
    // SAFETY: `PollFd` is `#[repr(C)]` with the exact field layout of `libc::pollfd`
    // (c_int, c_short, c_short on x86_64); `fds` is a valid mutable slice and
    // `timeout_ms` is a valid poll timeout.
    let n = cvt(unsafe {
        libc::poll(
            fds.as_mut_ptr().cast(),
            fds.len() as libc::nfds_t,
            timeout_ms,
        ) as isize
    })?;
    Ok(n as usize)
}

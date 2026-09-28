//! `listen` — mark a bound socket as passive, ready to accept connections.

use crate::{LocalFd, cvt};

/// Mark `fd` (a bound socket) as a listening socket. `backlog` caps the queue
/// of pending connections. Listening twice is allowed; `dgram` sockets are
/// rejected by the kernel (`EOPNOTSUPP`).
pub fn listen(fd: &LocalFd, backlog: i32) -> Result<(), crate::SyscallError> {
    // SAFETY: `fd` is a valid open socket; an invalid state (unbound socket,
    // dgram) returns an errno handled by `cvt`.
    cvt(unsafe { libc::listen(fd.as_raw(), backlog) as isize })?;
    Ok(())
}

// mutants: -C --test -C sockets -- -E 'binary(sockets)'
//! `listen` — mark a bound socket as passive, ready to accept connections.

use crate::{LocalFd, cvt};

impl LocalFd {
    /// Mark the bound socket `self` as a listening socket. `backlog` caps the queue
    /// of pending connections. Listening twice is allowed; `dgram` sockets are
    /// rejected by the kernel (`EOPNOTSUPP`).
    pub fn listen(&self, backlog: i32) -> Result<(), crate::SyscallError> {
        // SAFETY: `self` is a valid open socket; an invalid state (unbound socket,
        // dgram) returns an errno handled by `cvt`.
        cvt(unsafe { libc::listen(self.as_raw(), backlog) as isize })?;
        Ok(())
    }
}

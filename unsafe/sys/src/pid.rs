//! Process ID type for traceability.
//!
//! PIDs come exclusively from `fork()` return values and `getpid()`. This
//! newtype prevents accidental mixing with exit codes, fd numbers, or flags.

use core::fmt;

/// A process ID.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pid(libc::pid_t);

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Pid {
    pub const fn from_raw(raw: libc::pid_t) -> Self {
        Self(raw)
    }

    pub fn as_raw(self) -> libc::pid_t {
        self.0
    }
}

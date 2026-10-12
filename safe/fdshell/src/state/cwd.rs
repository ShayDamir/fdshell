use sys::LocalFd;
use sys::ShortCStr;

use super::ShellState;

impl ShellState {
    /// The shell's pinned `%CWD` directory fd — strict mode's resolution base
    /// for redirect targets and the PATH search (it follows `cd`). `None`
    /// only before startup seeds it (defensive: callers fall back to the
    /// process CWD).
    pub(crate) fn cwd_fd(&self) -> Option<&LocalFd> {
        self.fds.get::<ShortCStr>(&c"CWD".into()).map(|v| &v.fd)
    }
}

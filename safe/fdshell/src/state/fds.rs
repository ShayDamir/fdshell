//! The descriptor-value types of `ShellState`'s fd tables: an fd variable
//! (`%name`) and an fd array element (`%arr[k]`), each owning a descriptor
//! with its provenance trace.

// mutants: -C --lib -- -E 'test(/^state::fds::/)'

use sys::LocalFd;
use sys::ShortCStr;
use sys::Trace;

use super::ShellState;

/// An fd variable: an owned descriptor together with its provenance trace.
pub struct FdVar {
    pub fd: LocalFd,
    pub trace: Trace,
}

/// An fd array element: an owned descriptor, the name of the variable or
/// SHELLFD tag it originated from (its provenance), and its trace.
pub struct FdArrayEntry {
    pub fd: LocalFd,
    pub source: ShortCStr,
    pub trace: Trace,
}

impl ShellState {
    /// The fd variable `%name` holds, `None` when it is unset. The read half of
    /// [`ShellState::set_fd_var`]; callers map `None` to their own error.
    pub(crate) fn fd_var(&self, name: &ShortCStr) -> Option<&FdVar> {
        self.fds.get(name)
    }
}

#[cfg(test)]
mod tests;

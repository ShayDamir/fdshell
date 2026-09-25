//! Close the pipeline fds a forked child inherited but does not use.
//!
//! Every child inherits the parent's whole fd table: all pipeline pipe
//! ends, all capture socketpairs, all earlier siblings' pidfds. External
//! stages shed all of them at `exec` (each carries CLOEXEC), but a builtin
//! stage never execs, so it would keep the upstream write ends open for
//! the whole builtin run — readers on those pipes never see EOF, which
//! deadlocks a pipeline whose builtin stage reads stdin. Close every
//! inherited pipe end except the two this stage cloned into its redirects,
//! every sibling capture pair, and every sibling pidfd, before the stage's
//! command runs.
//!
//! The closes go by number: in the child, the `LocalFd` values in `pipes`,
//! `capture_pairs` and `children` (copies of the parent's stack locals) are
//! never dropped — every path after `run_child` returns exits via
//! `sys::exit`, which skips destructors — so closing the raw numbers
//! cannot double-close.

use error_stack::{Report, ResultExt};
use sys::{LocalFd, Pid};

use crate::error::child_process::ChildProcessError;

fn close_fd(fd: &LocalFd) -> Result<(), Report<ChildProcessError>> {
    sys::dup::close(fd.as_raw()).change_context(ChildProcessError::InheritedFdClose)
}

pub fn close_inherited(
    pipes: &[(LocalFd, LocalFd)],
    capture_pairs: &[Option<(LocalFd, LocalFd)>],
    children: &[(Pid, LocalFd)],
    i: usize,
) -> Result<(), Report<ChildProcessError>> {
    for (read_end, write_end) in pipes {
        close_fd(read_end)?;
        close_fd(write_end)?;
    }
    for (j, pair) in capture_pairs.iter().enumerate() {
        if j == i {
            continue;
        }
        if let Some((a, b)) = pair {
            close_fd(a)?;
            close_fd(b)?;
        }
    }
    for (_, pidfd) in children {
        close_fd(pidfd)?;
    }
    Ok(())
}

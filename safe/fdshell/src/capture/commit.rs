use alloc::vec::Vec;
use error_stack::Report;

use crate::error::capture::CaptureError;
use crate::state::{FdArrayEntry, FdVar, ShellState};
use sys::{LocalFd, ShortCStr};

use super::{Capture, do_captures};

/// A received fd: the target var and the value it carries.
pub struct CapturedFd {
    pub var: ShortCStr,
    pub value: CapturedValue,
}

pub enum CapturedValue {
    Fd(FdVar),
    Array(Vec<FdArrayEntry>),
}

/// Commit captured fds into state: scalars into `fds`, entries into `arrays`.
pub fn commit_captured(state: &mut ShellState, captured: Vec<CapturedFd>) {
    for c in captured {
        match c.value {
            CapturedValue::Fd(fdvar) => state.set_fd_var(c.var, fdvar),
            CapturedValue::Array(entries) => state.commit_captured_array(c.var, entries),
        }
    }
}

/// Receive, match and commit captures in one step.
pub fn capture_and_commit(
    capture_fd: LocalFd,
    expected_pid: sys::Pid,
    captures: Vec<Capture>,
    state: &mut ShellState,
) -> Result<(), Report<CaptureError>> {
    let captured = do_captures(capture_fd, expected_pid, captures, state)?;
    commit_captured(state, captured);
    Ok(())
}

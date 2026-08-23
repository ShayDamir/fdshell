use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};

use crate::error::capture::CaptureError;
use crate::state::{FdVar, ShellState};
use sys::{Origin, Position, ShortCStr, Trace};

#[cfg(test)]
mod tests;

// Clone required by pipeline/mod.rs (cmd.captures.clone()).
// Debug + PartialEq are test-only — quarantined behind cfg_attr.
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Capture {
    pub var: ShortCStr,
    pub tag: Option<ShortCStr>,
    pub force: bool,
    /// Position of the capture token in the source line.
    pub set_at: Position,
}

/// Receive fds from `capture_fd`, match against captures, stage results.
///
/// Returns a `Vec` of `(var, FdVar)` pairs on success. Each fd carries a
/// trace: the capture position plus the sender's SHELLFD tag as origin.
/// The caller commits them atomically into the state's fds.
pub fn do_captures(
    capture_fd: sys::LocalFd,
    expected_pid: sys::Pid,
    captures: Vec<Capture>,
    state: &ShellState,
) -> Result<Vec<(ShortCStr, FdVar)>, Report<CaptureError>> {
    let mut captured_fds: Vec<(ShortCStr, FdVar)> = Vec::with_capacity(captures.len());
    let mut remaining = captures;

    while !remaining.is_empty() {
        let mut buf = [0u8; sys::shellfd::TAG_MAX];
        let (fd, rtag) = match sys::shellfd::recv_fd(&capture_fd, &mut buf, expected_pid) {
            Ok(v) => v,
            Err(e) => {
                let ctx = e.current_context();
                if matches!(*ctx, sys::RecvFdError::Closed) {
                    break;
                }
                return Err(e).change_context(CaptureError::ReceiveFailed);
            }
        };
        if crate::last_arg::is_tag(rtag) {
            continue;
        }
        let idx = remaining
            .iter()
            .position(|c| c.tag.as_ref().is_some_and(|t| t.eq_bytes(rtag.to_bytes())))
            .or_else(|| remaining.iter().position(|c| c.tag.is_none()));
        if let Some(i) = idx {
            debug_assert!(i < remaining.len());
            let c = remaining.remove(i);
            if !c.force && state.fds.contains_key(&c.var) {
                bail!(CaptureError::Exists);
            }
            captured_fds.push((
                c.var,
                FdVar {
                    fd,
                    trace: Trace::at(c.set_at, Origin::Captured(tag_name(rtag))),
                },
            ));
        }
    }

    if !remaining.is_empty() {
        bail!(CaptureError::Incomplete {
            expected: remaining.len() + captured_fds.len(),
            received: captured_fds.len(),
        });
    }

    Ok(captured_fds)
}

/// A received SHELLFD tag is NUL-free up to its terminator, so `push` is infallible.
fn tag_name(rtag: &CStr) -> ShortCStr {
    let mut name = ShortCStr::new();
    name.push(rtag);
    name
}

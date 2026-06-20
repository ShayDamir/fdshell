#![forbid(unsafe_code)]

use crate::error::capture::CaptureError;
use crate::state::ShellState;
use sys::ShortCStr;

// Clone required by pipeline/mod.rs (cmd.captures.clone()).
// Debug + PartialEq are test-only — quarantined behind cfg_attr.
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Capture {
    pub var: ShortCStr,
    pub tag: Option<ShortCStr>,
    pub force: bool,
}

/// Receive fds from `capture_fd`, match against captures, stage results.
///
/// Returns a `Vec` of `(var, fd)` pairs on success. The caller commits
/// them atomically into the state's fds.
pub fn do_captures(
    capture_fd: sys::LocalFd,
    expected_pid: i32,
    captures: Vec<Capture>,
    state: &ShellState,
) -> Result<Vec<(ShortCStr, sys::LocalFd)>, CaptureError> {
    let mut captured_fds: Vec<(ShortCStr, sys::LocalFd)> = Vec::with_capacity(captures.len());
    let mut remaining = captures;

    while !remaining.is_empty() {
        let mut buf = [0u8; sys::shellfd::TAG_MAX];
        let (fd, rtag) = match sys::shellfd::recv_fd(&capture_fd, &mut buf, expected_pid) {
            Err(e) if e == sys::errno::EAGAIN => break,
            Err(_) => return Err(CaptureError::ReceiveFailed),
            Ok(v) => v,
        };
        let idx = remaining
            .iter()
            .position(|c| c.tag.as_ref().is_some_and(|t| t.eq_bytes(rtag.to_bytes())))
            .or_else(|| remaining.iter().position(|c| c.tag.is_none()));
        if let Some(i) = idx {
            debug_assert!(i < remaining.len());
            let c = remaining.remove(i);
            if !c.force && state.fds.contains_key(&c.var) {
                return Err(CaptureError::Exists);
            }
            captured_fds.push((c.var, fd));
        }
    }

    Ok(captured_fds)
}

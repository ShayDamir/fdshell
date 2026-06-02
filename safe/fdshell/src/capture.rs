#![forbid(unsafe_code)]

use crate::vars::FdVars;
use sys::ShortCStr;
use sys::errno::EEXIST;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Capture {
    pub var: ShortCStr,
    pub tag: Option<ShortCStr>,
    pub force: bool,
}

/// Receive fds from `capture_fd`, match against captures, stage results.
/// Receive fds from `capture_fd`, match against captures, stage results.
///
/// Returns a `Vec` of `(var, fd)` pairs on success. The caller commits
/// them atomically into `fdvars`.
///
/// # Assumptions
///
/// - `captures` has unique target variables (parser guarantees this).
///   Duplicate targets would break the `EEXIST` check against committed state.
/// - Captures are positional if untagged, matched by tag if tagged.
///   Unknown fds (no matching capture) are silently closed.
pub fn do_captures(
    capture_fd: sys::LocalFd,
    expected_pid: i32,
    captures: Vec<Capture>,
    fdvars: &FdVars,
) -> Result<Vec<(ShortCStr, sys::LocalFd)>, i32> {
    let mut captured_fds: Vec<(ShortCStr, sys::LocalFd)> = Vec::with_capacity(captures.len());
    let mut remaining = captures;

    while !remaining.is_empty() {
        let mut buf = [0u8; sys::shellfd::TAG_MAX];
        let (fd, rtag) = match sys::shellfd::recv_fd(&capture_fd, &mut buf, expected_pid) {
            Err(e) if e == sys::errno::EAGAIN => break,
            Err(e) => return Err(e),
            Ok(v) => v,
        };
        let idx = remaining
            .iter()
            .position(|c| c.tag.as_ref().is_some_and(|t| t.eq_bytes(rtag.to_bytes())))
            .or_else(|| remaining.iter().position(|c| c.tag.is_none()));
        if let Some(i) = idx {
            debug_assert!(i < remaining.len());
            let c = remaining.remove(i);
            if !c.force && fdvars.resolve(&c.var).is_some() {
                return Err(EEXIST);
            }
            captured_fds.push((c.var, fd));
        }
    }

    Ok(captured_fds)
}

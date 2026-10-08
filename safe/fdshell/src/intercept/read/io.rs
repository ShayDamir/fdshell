use crate::error::cmd::CmdError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ImportedFd;
use sys::poll::{POLLIN, PollFd};

use super::flags::{ReadFlags, SourceFd};
use super::line::{Line, LineEnd};
use super::read_from_fd::read_line_from_fd;

/// Read one line from `source` into a fresh [`Line`].
///
/// The `-t` gate is a one-shot `poll`: bash re-polls per byte, so a peer that
/// stalls mid-line past the timeout still blocks here; with data ready the
/// read proceeds to completion like bash.
pub(crate) fn read_line(
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
    flags: &ReadFlags,
) -> Result<(Vec<u8>, LineEnd), Report<CmdError>> {
    if let Some(ms) = flags.timeout_ms()
        && let Some(fd) = source_raw_fd(source, fd_clone)?
        && poll_unready(fd, ms)?
    {
        return Ok((Vec::new(), LineEnd::Timeout));
    }

    let mut line = Line::new(flags.delim, flags.raw, flags.max_bytes);
    match source {
        SourceFd::Stdin => read_line_from_fd(|b: &mut [u8]| sys::IN.read(b), &mut line)?,
        SourceFd::RawFd(fd_arg) => {
            let fd = ImportedFd::try_from(fd_arg).change_context(CmdError::Read)?;
            read_line_from_fd(|b: &mut [u8]| fd.read(b), &mut line)?;
        }
        SourceFd::FdVar(_) => {
            if let Some(local) = fd_clone {
                read_line_from_fd(|b: &mut [u8]| local.read(b), &mut line)?;
            }
        }
    }
    let end = line.end();
    Ok((line.buf, end))
}

/// The pollable raw fd behind `source`, if any (a clone-less FdVar has none).
fn source_raw_fd(
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
) -> Result<Option<i32>, Report<CmdError>> {
    let raw = match source {
        SourceFd::Stdin => Some(sys::IN.as_raw()),
        SourceFd::RawFd(fd_arg) => Some(
            ImportedFd::try_from(fd_arg)
                .change_context(CmdError::Read)?
                .as_raw(),
        ),
        SourceFd::FdVar(_) => fd_clone.map(|local| local.as_raw()),
    };
    Ok(raw)
}

/// Poll `fd` for input for up to `ms`; true when nothing is ready (timeout).
fn poll_unready(fd: i32, ms: i32) -> Result<bool, Report<CmdError>> {
    let mut fds = [PollFd::new(fd, POLLIN)];
    let n = sys::poll::poll(&mut fds, ms).change_context(CmdError::Read)?;
    Ok(n == 0)
}

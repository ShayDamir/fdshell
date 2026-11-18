// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

use crate::error::cmd::CmdError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ImportedFd;
use sys::poll::{POLLIN, PollFd};

use super::flags::{ReadFlags, SourceFd};
use super::line::{Line, LineEnd};
use super::read_from_fd::read_line_from_fd;

/// `poll` timeout meaning "block forever" (`read -t` values that saturate).
const INFINITE_MS: i32 = -1;

/// Read one line from `source` into a fresh [`Line`].
///
/// The `-t` gate is a `poll`: bash re-polls per byte, so a peer that stalls
/// mid-line past the timeout still blocks here; with data ready the read proceeds
/// to completion like bash.
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
///
/// A handled signal runs its trap and answers `EINTR` (no `SA_RESTART`), and the
/// poll resumes with the time that is left, so the deadline still holds — measured
/// 2026-11-17 (task #581's third bug): bash's `read -t 1` with a trap that fires
/// mid-wait answers the *timeout*, not an error, and the script keeps going.
fn poll_unready(fd: i32, ms: i32) -> Result<bool, Report<CmdError>> {
    let deadline = deadline_ms(ms)?;
    let mut fds = [PollFd::new(fd, POLLIN)];
    loop {
        let wait = match deadline {
            None => INFINITE_MS,
            Some(at) => remaining_ms(at)?,
        };
        match sys::poll::poll(&mut fds, wait) {
            Ok(0) => return Ok(true),
            Ok(_) => return Ok(false),
            Err(err) => {
                // A handled signal answers EINTR: run the trap and re-poll with the time
                // left. No other errno is reachable from this call, so the report below is
                // the answer a real failure gets (the operator-free form keeps the rule
                // without a mutant site — CATCHING_MUTANTS rule 30).
                if matches!(err.errno(), sys::errno::EINTR) {
                    continue;
                }
                return Err(Report::new(err).change_context(CmdError::Read));
            }
        }
    }
}

/// The monotonic instant `ms` from now; `None` is the infinite (`-1`) poll.
fn deadline_ms(ms: i32) -> Result<Option<u64>, Report<CmdError>> {
    if ms < 0 {
        return Ok(None);
    }
    Ok(Some(
        sys::clock::monotonic_us().change_context(CmdError::Read)? + (ms as u64) * 1000,
    ))
}

/// Milliseconds left before `deadline`; never negative, so the poll stays legal.
fn remaining_ms(deadline: u64) -> Result<i32, Report<CmdError>> {
    let now = sys::clock::monotonic_us().change_context(CmdError::Read)?;
    Ok((deadline.saturating_sub(now) / 1000) as i32)
}

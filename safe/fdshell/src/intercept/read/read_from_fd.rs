// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

use crate::error::cmd::CmdError;
use crate::fd_read::{Decision, read_to_end};
use error_stack::Report;
use sys::SyscallError;

use super::line::Line;

/// Read from `read` into `line` until the line is finished or the input ends.
pub(crate) fn read_line_from_fd(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, SyscallError>,
    line: &mut Line,
) -> Result<(), Report<CmdError>> {
    if line.finished() {
        // `-n 0` finishes the line before any byte is read (the shared loop always
        // reads once before it asks the sink), so the loop is not entered at all.
        return Ok(());
    }
    read_to_end(
        &mut read,
        &mut |chunk| {
            for &b in chunk {
                line.feed(b);
            }
            if line.finished() {
                Decision::Done
            } else {
                Decision::Resume
            }
        },
        // A caught signal's handler has run and the input is still open, so the
        // read restarts (bash's rule, measured 2026-11-17 — task #581's second bug).
        // Any other errno is the site's own report, exactly as before.
        &|err| {
            if err.errno() == sys::errno::EINTR {
                Decision::Resume
            } else {
                Decision::Report(Report::new(*err).change_context(CmdError::Read))
            }
        },
        &|| Decision::Report(Report::new(CmdError::Never)),
    )
}

//! Reading one line from the REPL's stdin, with the `ignoreeof` EOF policy.

mod reader;

pub(crate) use reader::LineReader;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// Write `prompt`, then read one line from `sys::IN` into `buf` (up to, not
/// including, the terminating `\n`). Returns whether the REPL should keep
/// reading: `true` on a line, or on EOF with `ignoreeof` on in an
/// interactive (terminal) loop; `false` on EOF with `ignoreeof` off, and on
/// EOF with it on over a pipe (the hint is printed either way). Fails with
/// [`AppError::LineTooLarge`] once `buf` would grow past `limit` bytes; the
/// caller reuses `buf` across the continuation loop, so the cap also bounds
/// the whole accumulated multi-line construct.
pub(crate) fn read_line(
    cell: &ForkCell<ShellState>,
    buf: &mut Vec<u8>,
    prompt: &[u8],
    limit: usize,
    input: &mut LineReader,
) -> Result<bool, Report<AppError>> {
    sys::OUT.write_all(prompt).change_context(AppError::Read)?;
    if input.read_into(buf, limit)? {
        Ok(true)
    } else {
        eof_continues(cell)
    }
}

/// End of input: with `ignoreeof` on, hint how to leave, and keep the shell
/// alive only in the interactive (terminal) loop — a pipe's EOF is sticky, so
/// continuing there livelocks. Otherwise exit (bash, task #291).
pub(crate) fn eof_continues(cell: &ForkCell<ShellState>) -> Result<bool, Report<AppError>> {
    let (ignoreeof, interactive) = {
        let state = cell.borrow().change_context(AppError::Borrow)?;
        (
            state.options & crate::options::IGNOREEOF != 0,
            state.interactive,
        )
    };
    if !ignoreeof {
        return Ok(false);
    }
    // The shared `Ref` above is dropped: `write_all` is a syscall.
    sys::OUT
        .write_all(b"use `exit' to leave\n")
        .change_context(AppError::Read)?;
    Ok(interactive)
}

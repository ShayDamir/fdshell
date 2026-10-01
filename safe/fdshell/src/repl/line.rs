//! Reading one line from the REPL's stdin, with the `ignoreeof` EOF policy.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// Write `prompt`, then read one line from `sys::IN` into `buf` (up to, not
/// including, the terminating `\n`). Returns whether the REPL should keep
/// reading: `true` on a line, or on EOF with `ignoreeof` on (the hint is
/// printed); `false` on EOF with `ignoreeof` off.
pub(crate) fn read_line(
    cell: &ForkCell<ShellState>,
    buf: &mut Vec<u8>,
    prompt: &[u8],
) -> Result<bool, Report<AppError>> {
    sys::OUT.write_all(prompt).change_context(AppError::Read)?;
    let mut byte = [0u8; 1];
    loop {
        let n = sys::IN.read(&mut byte).change_context(AppError::Read)?;
        if n == 0 {
            return eof_continues(cell);
        }
        if byte[0] == b'\n' {
            return Ok(true);
        }
        buf.push(byte[0]);
    }
}

/// End of input: with `ignoreeof` on, hint how to leave and keep the shell
/// alive (bash compat); otherwise exit.
pub(crate) fn eof_continues(cell: &ForkCell<ShellState>) -> Result<bool, Report<AppError>> {
    let state = cell.borrow().change_context(AppError::Borrow)?;
    if state.options & crate::options::IGNOREEOF == 0 {
        return Ok(false);
    }
    sys::OUT
        .write_all(b"use `exit' to leave\n")
        .change_context(AppError::Read)?;
    Ok(true)
}

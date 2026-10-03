//! Reading one line from the REPL's stdin, with the `ignoreeof` EOF policy.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::app::AppError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// Buffered stdin shared across `read_line` calls: a 4 KiB chunk can end
/// mid-line, and bytes after a `\n` belong to the next line (a pipe has no
/// pushback), so reads keep a persistent position. Byte-at-a-time reads
/// cost one syscall per byte — ~5 s for a 64 MiB line on a fast host,
/// past the nextest slow-timeout on slow ones.
pub(crate) struct LineReader {
    chunk: [u8; 4096],
    pos: usize,
    len: usize,
}

impl LineReader {
    pub(crate) fn new() -> Self {
        Self {
            chunk: [0; 4096],
            pos: 0,
            len: 0,
        }
    }

    /// Refill the chunk when exhausted; `false` on end of input.
    fn fill(&mut self) -> Result<bool, Report<AppError>> {
        if self.pos < self.len {
            return Ok(true);
        }
        self.len = sys::IN
            .read(&mut self.chunk)
            .change_context(AppError::Read)?;
        self.pos = 0;
        Ok(self.len > 0)
    }

    /// The unconsumed part of the chunk; `pos..len` is kept in range, so
    /// `None` is an internal invariant breach.
    fn avail(&self) -> Result<&[u8], Report<AppError>> {
        self.chunk
            .get(self.pos..self.len)
            .ok_or_else(|| Report::new(AppError::Never))
    }
}

/// Write `prompt`, then read one line from `sys::IN` into `buf` (up to, not
/// including, the terminating `\n`). Returns whether the REPL should keep
/// reading: `true` on a line, or on EOF with `ignoreeof` on (the hint is
/// printed); `false` on EOF with `ignoreeof` off. Fails with
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
    loop {
        if !input.fill()? {
            return eof_continues(cell);
        }
        let avail = input.avail()?;
        match avail.iter().position(|&b| b == b'\n') {
            Some(i) => {
                let head = avail.get(..i).ok_or(AppError::Never)?;
                append_capped(buf, head, limit)?;
                input.pos += i + 1;
                return Ok(true);
            }
            None => {
                append_capped(buf, avail, limit)?;
                input.pos = input.len;
            }
        }
    }
}

/// Extend `buf` with `take`, failing once the line would grow past `limit`
/// bytes: a line of exactly `limit` bytes is allowed, the next one fails
/// (same off-by-one convention as `cli::load_script`).
fn append_capped(buf: &mut Vec<u8>, take: &[u8], limit: usize) -> Result<(), Report<AppError>> {
    if buf.len() + take.len() > limit {
        bail!(AppError::LineTooLarge);
    }
    buf.extend_from_slice(take);
    Ok(())
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

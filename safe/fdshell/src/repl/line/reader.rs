// mutants: -C --lib -C --test -C repl -C --test -C read -- -E 'test(/^repl::/) | binary(repl) | binary(read)'

//! The buffered chunk reader behind `read_line`: 4 KiB reads and the capped
//! accumulation of one line's bytes. On a stdin-fed session it shares the
//! stdin buffer with the `read` builtin (task #630) through the two rules in
//! [`shared`]: consult the buffer before any syscall, and yield what a line
//! did not take to it.

mod shared;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::app::AppError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

use shared::{drain_shared, put_back_shared};

/// Buffered stdin shared across `read_line` calls and, since task #630, with
/// the `read` builtin's buffer through `shared`: a 4 KiB chunk can span two
/// lines, and bytes after a `\n` belong to the next line — a pipe has no
/// pushback, so the shared buffer, not this reader, is what persists across
/// calls (the chunk is refilled from its head every time). Byte-at-a-time
/// reads cost one syscall per byte — ~5 s for a 64 MiB line on a fast host,
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

    /// Refill the chunk from stdin; `false` on end of input. The chunk is
    /// empty on entry (task #630): a line's tail lives in the shared buffer,
    /// never in this reader.
    fn fill(&mut self) -> Result<bool, Report<AppError>> {
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

    /// Advance the position past `n` consumed bytes — within one `read_into`
    /// call only (task #630): what the line does not take goes to the shared
    /// buffer, never to a resting position in this reader.
    fn consume(&mut self, n: usize) {
        self.pos += n;
    }

    /// Read from stdin into `buf` up to, not including, the terminating
    /// `\n`: `true` when a line completed, `false` on end of input. Fails
    /// with [`AppError::LineTooLarge`] once `buf` would grow past `limit`.
    /// The shared buffer answers before any syscall; what a completed line
    /// did not take from a syscall chunk goes back to it, so nothing stays
    /// private here across calls and the builtin's drain-first reader sees
    /// everything.
    pub(crate) fn read_into(
        &mut self,
        cell: &ForkCell<ShellState>,
        buf: &mut Vec<u8>,
        limit: usize,
    ) -> Result<bool, Report<AppError>> {
        if drain_shared(cell, buf, limit)? {
            return Ok(true);
        }
        loop {
            if !self.fill()? {
                return Ok(false);
            }
            let avail = self.avail()?;
            match avail.iter().position(|&b| b == b'\n') {
                Some(i) => {
                    let head = avail.get(..i).ok_or(AppError::Never)?;
                    append_capped(buf, head, limit)?;
                    self.consume(i + 1);
                    let tail = self.avail()?;
                    put_back_shared(cell, tail)?;
                    return Ok(true);
                }
                None => {
                    append_capped(buf, avail, limit)?;
                }
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

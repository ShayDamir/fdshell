//! The buffered chunk reader behind `read_line`: 4 KiB reads with a
//! persistent position, and the capped accumulation of one line's bytes.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::app::AppError;

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

    /// Advance the position past `n` consumed bytes.
    fn consume(&mut self, n: usize) {
        self.pos += n;
    }

    /// Consume the whole chunk.
    fn consume_all(&mut self) {
        self.pos = self.len;
    }

    /// Read from stdin into `buf` up to, not including, the terminating
    /// `\n`: `true` when a line completed, `false` on end of input. Fails
    /// with [`AppError::LineTooLarge`] once `buf` would grow past `limit`.
    pub(crate) fn read_into(
        &mut self,
        buf: &mut Vec<u8>,
        limit: usize,
    ) -> Result<bool, Report<AppError>> {
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
                    return Ok(true);
                }
                None => {
                    append_capped(buf, avail, limit)?;
                    self.consume_all();
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

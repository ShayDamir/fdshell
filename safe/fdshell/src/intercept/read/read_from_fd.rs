// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

//! The `read` line path's read loop: the shared `read_to_end` body driven by a
//! drain-first reader and a putback sink (task #609). The buffer's bookkeeping —
//! its key, its identity check, the state borrows — is [`super::putback`].

use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::fd_read::{Decision, read_to_end};
use error_stack::Report;
use sys::SyscallError;

use super::line::Line;

/// Read from `read` into `line` until the line is finished or the input ends.
///
/// `pending` is the source's putback buffer: its bytes are served **before** any
/// syscall, and what the line did not consume is left back in it (task #609).
pub(crate) fn read_line_from_fd(
    mut syscall: impl FnMut(&mut [u8]) -> Result<usize, SyscallError>,
    pending: &mut Vec<u8>,
    line: &mut Line,
) -> Result<(), Report<CmdError>> {
    if line.finished() {
        // `-n 0` finishes the line before any byte is read (the shared loop always
        // reads once before it asks the sink), so the loop is not entered at all.
        return Ok(());
    }
    let mut tail = Vec::new();
    {
        // `read_to_end` takes two `&mut dyn FnMut` arguments, so the two buffers
        // split across the closures: the reader owns the pending one, the sink
        // collects into its own local, and the write-back follows the loop.
        let mut reader = |buf: &mut [u8]| drain_first(pending, buf, &mut syscall);
        let mut sink = |chunk: &[u8]| putback(line, chunk, &mut tail);
        read_to_end(
            &mut reader,
            &mut sink,
            // A caught signal's handler has run and the input is still open, so the
            // read restarts (bash's rule, measured 2026-11-17 — task #581's second
            // bug). Any other errno is the site's own report, exactly as before.
            &|err| {
                if err.errno() == sys::errno::EINTR {
                    Decision::Resume
                } else {
                    Decision::Report(Report::new(*err).change_context(CmdError::Read))
                }
            },
            &|| Decision::Report(Report::new(CmdError::Never)),
        )?;
    }
    *pending = tail;
    Ok(())
}

/// Serve the buffered bytes first, so a line completed by them needs no syscall at
/// all — exactly what bash does with bytes already in its buffer. Only an empty
/// buffer reaches the real read, which is how EOF is still learned from a `read`
/// returning 0. The buffer never holds more than one chunk, so it hands over all
/// of itself.
fn drain_first(
    pending: &mut Vec<u8>,
    buf: &mut [u8],
    syscall: &mut impl FnMut(&mut [u8]) -> Result<usize, SyscallError>,
) -> Result<usize, SyscallError> {
    if pending.is_empty() {
        return syscall(buf);
    }
    let take = pending.len();
    {
        let (head, _) = pending.split_at_mut(take);
        let (dst, _) = buf.split_at_mut(take);
        dst.copy_from_slice(head);
    }
    pending.clear();
    Ok(take)
}

/// Feed `chunk` to `line` byte by byte and collect the tail in `tail`: the byte
/// that finished the line (a delimiter byte, or the byte that reached the `-n`
/// cap) is the line's, everything after it belongs to the next `read`. Feeding
/// past `done` is a no-op, so a single read behaves exactly as it did before the
/// buffer existed — only the leftover survives now.
fn putback(line: &mut Line, chunk: &[u8], tail: &mut Vec<u8>) -> Decision<CmdError> {
    for (index, &byte) in chunk.iter().enumerate() {
        if line.finished() {
            if let Some(rest) = chunk.get(index..) {
                tail.extend_from_slice(rest);
            }
            break;
        }
        line.feed(byte);
    }
    if line.finished() {
        Decision::Done
    } else {
        Decision::Resume
    }
}

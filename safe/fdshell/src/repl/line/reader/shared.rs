// mutants: -C --lib -C --test -C repl -C --test -C read -- -E 'test(/^repl::/) | binary(repl) | binary(read)'

//! The shared stdin buffer, parse side (task #630): the two join rules —
//! consult the buffer before any syscall, and yield what a line did not take
//! to it — so at every yield point all unassigned stdin bytes live in the
//! buffer and the parse layer and the `read` builtin see one stream, as in
//! bash. The bookkeeping is [`crate::state::read_pending`], reached through
//! the same state methods as the builtin side, so the descriptor-identity
//! rule of `intercept/read/putback.rs` binds the parse side too. Each borrow
//! is statement-wide and the identity closure runs only when the state has
//! something to check, so an unbuffered read pays no syscall.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::state::ShellState;
use crate::state::read_pending::{Identity, ReadKey};
use sys::fork_cell::ForkCell;

use super::append_capped;

/// The key of the shared stdin buffer — the fd number as the script spells
/// stdin, exactly the one `intercept::read` uses, so `read x`, `read -u 0 x`
/// and the parse layer are one stream, as they are in bash.
fn stdin_key() -> ReadKey {
    ReadKey::Fd(c"0".into())
}

/// The identity of stdin — `fstat`'s `dev` + `ino`, computed as
/// `intercept/read/putback.rs` computes it: a descriptor reassigned under the
/// buffer (`exec <`) drops it, through the state methods, not a re-implemented
/// rule.
fn stdin_identity() -> Option<Identity> {
    let stat = sys::IN.fstat().ok()?;
    Some(Identity {
        dev: stat.dev,
        ino: stat.ino,
    })
}

/// The shared buffer's bytes, **taken** from the state — one reader per
/// buffer, empty when nothing is buffered.
fn take_shared(cell: &ForkCell<ShellState>) -> Result<Vec<u8>, Report<AppError>> {
    let key = stdin_key();
    Ok(cell
        .borrow_mut()
        .change_context(AppError::Borrow)?
        .take_read_pending(&key, stdin_identity))
}

/// Rule one: drain the shared buffer ahead of any syscall, appending its
/// bytes to `buf` under the cap rule. `true` when a buffered `\n` completes
/// the line and the bytes past it are handed back to the buffer for the next
/// line — take-once suffices, the buffer holds at most one chunk's tail, so a
/// buffered line without a `\n` lands in `buf` whole and the syscall loop
/// follows.
pub(crate) fn drain_shared(
    cell: &ForkCell<ShellState>,
    buf: &mut Vec<u8>,
    limit: usize,
) -> Result<bool, Report<AppError>> {
    let shared = take_shared(cell)?;
    if shared.is_empty() {
        return Ok(false);
    }
    let cut = shared.iter().position(|&b| b == b'\n');
    let head = shared
        .get(..cut.unwrap_or(shared.len()))
        .ok_or(AppError::Never)?;
    append_capped(buf, head, limit)?;
    let Some(i) = cut else {
        return Ok(false);
    };
    let tail = shared.get(i + 1..).ok_or(AppError::Never)?;
    put_back_shared(cell, tail)?;
    Ok(true)
}

/// Rule two: yield `tail` — what the completed line did not take — to the
/// shared buffer for the next reader on stdin. An empty tail stores nothing,
/// so the byte-for-byte cap path never routes through the buffer.
pub(crate) fn put_back_shared(
    cell: &ForkCell<ShellState>,
    tail: &[u8],
) -> Result<(), Report<AppError>> {
    let key = stdin_key();
    cell.borrow_mut()
        .change_context(AppError::Borrow)?
        .store_read_pending(key, stdin_identity, tail.to_vec());
    Ok(())
}

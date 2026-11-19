// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

//! The `read` line path's putback buffer, read side (task #609): which source a
//! buffered tail belongs to, the descriptor identity it is validated against, and
//! the two narrow state borrows — take the buffer before the read, put the
//! leftover back after it. The bookkeeping itself is [`crate::state::read_pending`];
//! the drain-first reader and the putback sink live in [`super::read_from_fd`].
//!
//! Each borrow is statement-wide only: nothing may hold a `ForkCell` borrow across
//! the read (LESSONS "A RefCell borrow conflict can masquerade as an unrelated
//! domain error"), and the identity is asked only when the state has something to
//! check, so an unbuffered read pays no extra syscall.

use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use crate::state::read_pending::{Identity, ReadKey};
use error_stack::{Report, ResultExt};
use sys::ImportedFd;
use sys::fork_cell::ForkCell;

use super::flags::SourceFd;

/// The namespace key of the buffer for `source`: the fd number as the script
/// spelled it — so stdin and `-u 0` are one source, as they are in bash — or the fd
/// var's **name**, because its per-call clone can land on a different number each
/// time.
pub(crate) fn pending_key(source: &SourceFd) -> ReadKey {
    match source {
        SourceFd::Stdin => ReadKey::Fd(c"0".into()),
        SourceFd::RawFd(num) => ReadKey::Fd(num.clone()),
        SourceFd::FdVar(var) => ReadKey::Var(var.clone()),
    }
}

/// The identity of the descriptor behind `source` (`fstat`'s `dev` + `ino`),
/// `None` when the query fails — a descriptor closed under us, so the buffer it
/// guards is dropped and the read behaves as it did before buffers existed.
pub(crate) fn source_identity(
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
) -> Option<Identity> {
    let stat = match source {
        SourceFd::Stdin => sys::IN.fstat(),
        SourceFd::RawFd(num) => ImportedFd::try_from(num).ok()?.fstat(),
        SourceFd::FdVar(_) => fd_clone?.fstat(),
    };
    let stat = stat.ok()?;
    Some(Identity {
        dev: stat.dev,
        ino: stat.ino,
    })
}

/// The bytes an earlier read left for this source, **taken** from the state — one
/// read per buffer — empty when nothing is buffered.
pub(crate) fn take_pending(
    cell: &ForkCell<ShellState>,
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
) -> Result<Vec<u8>, Report<CmdError>> {
    let key = pending_key(source);
    Ok(cell
        .borrow_mut()
        .change_context(CmdError::Read)?
        .take_read_pending(&key, || source_identity(source, fd_clone)))
}

/// Record what the line did not consume so the next read on this source answers it;
/// an empty tail stores nothing.
pub(crate) fn put_back(
    cell: &ForkCell<ShellState>,
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
    bytes: Vec<u8>,
) -> Result<(), Report<CmdError>> {
    let key = pending_key(source);
    cell.borrow_mut()
        .change_context(CmdError::Read)?
        .store_read_pending(key, || source_identity(source, fd_clone), bytes);
    Ok(())
}

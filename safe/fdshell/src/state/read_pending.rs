// mutants: -C --lib -C --test -C read -- -E 'test(/^state::read_pending::/) | test(/^intercept::read::/) | binary(read)'

//! The `read` line path's **putback buffer**: the bytes one `read(2)` chunk
//! carried past the end of the line, kept for the next `read` on the same source
//! (task #609). A pipe has no pushback, so once the kernel hands the bytes over
//! the shell owns them — bash's line reader keeps exactly these bytes, and
//! without them a second `read`, a `-n` cap, a `-d` split or a `while read` loop
//! loses everything the first line did not consume.
//!
//! The state layer owns only the **bookkeeping**: which source the bytes belong
//! to, and whether the descriptor behind that source is still the one they came
//! from. The reading itself stays in `intercept::read` (this module cannot name
//! `SourceFd` — `intercept::read` is private to `intercept`).

use alloc::vec::Vec;

use sys::ShortCStr;

use super::ShellState;

/// The source a buffer belongs to, two namespaces. The **fd number as the script
/// spelled it**: stdin is `"0"`, so `read x` and `read -u 0 x` share one buffer
/// the way they share bash's single fd-0 stream. The **fd var name**: `run_read`
/// clones the var's fd per call and the clone can land on a different number each
/// time, so the number cannot key it (the spelling alias `-u %foo` vs `-u 3`
/// therefore keeps two buffers — fdshell clones per call, bash does not).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ReadKey {
    Fd(ShortCStr),
    Var(ShortCStr),
}

/// The descriptor identity the bytes were taken from — `fstat`'s `dev` + `ino`,
/// the cheapest handle that survives a name. A stable name gets reassigned under
/// the buffer (`exec <`, `exec 3<`, a heredoc `dup2`, a scope restore, `unset %f`
/// followed by number reuse), and bytes from a different descriptor are no longer
/// this source's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) dev: u64,
    pub(crate) ino: u64,
}

/// Buffered bytes plus the identity they were taken from.
pub(crate) struct Entry {
    pub(crate) identity: Identity,
    pub(crate) bytes: Vec<u8>,
}

impl ShellState {
    /// The bytes already read from `key`'s source and not yet consumed; empty
    /// when nothing is buffered. The entry is **taken** — one read per buffer —
    /// and `live` (the descriptor's current identity) is asked only when an entry
    /// exists, so the unbuffered path pays no syscall. A mismatch drops the entry
    /// and the read behaves exactly as it did before buffers existed; so does a
    /// `None` from `live` (the descriptor was closed under us).
    pub(crate) fn take_read_pending(
        &mut self,
        key: &ReadKey,
        live: impl FnOnce() -> Option<Identity>,
    ) -> Vec<u8> {
        let Some(entry) = self.read_pending.remove(key) else {
            return Vec::new();
        };
        if Some(entry.identity) != live() {
            return Vec::new();
        }
        entry.bytes
    }

    /// Record what the line did not consume so the next `read` on this source
    /// answers it. An empty tail stores nothing — the map holds only live buffers
    /// — and `live` is asked only when there is something to record. One chunk's
    /// tail per source bounds the memory, so nothing here needs a cap.
    pub(crate) fn store_read_pending(
        &mut self,
        key: ReadKey,
        live: impl FnOnce() -> Option<Identity>,
        bytes: Vec<u8>,
    ) {
        if bytes.is_empty() {
            return;
        }
        let Some(identity) = live() else {
            return;
        };
        self.read_pending.insert(key, Entry { identity, bytes });
    }
}

#[cfg(test)]
mod tests;

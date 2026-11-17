// mutants: -C --lib -C --test -C cmd_subst -C --test -C read -C --test -C source -C --test -C last_arg -- -E 'test(/^fd_read::/) | test(/^cmd_subst::/) | test(/^intercept::read::tests::/) | test(/^intercept::source::tests::/) | test(/^last_arg::/) | binary(cmd_subst) | binary(read) | binary(source) | binary(last_arg)'

//! THE chunked read-to-EOF body for every fd consumer, so no site re-implements
//! it and drifts (LESSONS "Two near-duplicate read loops diverge silently: one
//! lost its `eof` flag"). The body is written once: read a chunk, hand it to the
//! sink, stop on EOF or when the sink is done. What stays per-site is the
//! *decision*, because the trap handler has no `SA_RESTART` (task #582) and a
//! caught signal therefore answers `EINTR` (task #585). The policy table, each
//! row pinned: the `$( … )` capture and the `read` builtin **resume** the read
//! (bash's rule, measured 2026-11-17 — `tests/trap.rs`), `source` and `last_arg`
//! **report** it, and the `wait` block keeps its own chain
//! (`a_signal_during_the_blocking_poll_reports_its_own_chain`).

use core::error::Error;
use error_stack::Report;
use sys::SyscallError;

/// The most a single read syscall can fill — the size every site's buffer had
/// before the loop was shared.
const CHUNK: usize = 4096;

/// A site's answer to one event of the loop: the sink answers for the chunk it
/// was handed (its own cap included), `on_error` for a failed syscall.
pub(crate) enum Decision<E> {
    /// Read again, keeping what has been collected: the handler has run and the
    /// input is still open — the capture's and `read`'s rule (task #581).
    Resume,
    /// Finish with what has been collected: EOF, a finished line or cap, or the
    /// capture's old rule that a read error ends it quietly.
    Done,
    /// Stop and propagate this report — the site's own chain (STYLE §4).
    Report(Report<E>),
}

/// Read in `CHUNK` pieces, handing every chunk to `sink`, until EOF, the sink's
/// stop, or the error decision. `on_error` is this site's answer to a failed
/// syscall; `on_never` is its own `Never` for the slice a read cannot produce
/// (STYLE §4.11 — the loop keeps no error type of its own).
pub(crate) fn read_to_end<E>(
    read: &mut dyn FnMut(&mut [u8]) -> Result<usize, SyscallError>,
    sink: &mut dyn FnMut(&[u8]) -> Decision<E>,
    on_error: &dyn Fn(&SyscallError) -> Decision<E>,
    on_never: &dyn Fn() -> Decision<E>,
) -> Result<(), Report<E>>
where
    E: Error + Send + Sync + 'static,
{
    let mut buf = [0u8; CHUNK];
    loop {
        let decision = match read(&mut buf) {
            Ok(0) => Decision::Done,
            Ok(n) => match buf.get(..n) {
                Some(chunk) => sink(chunk),
                None => on_never(),
            },
            Err(err) => on_error(&err),
        };
        match decision {
            Decision::Resume => continue,
            Decision::Done => return Ok(()),
            Decision::Report(report) => return Err(report),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::indexing_slicing)]
mod tests;

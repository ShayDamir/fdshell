//! The `noclobber` decision shared by both redirect-open routes (`cwd.rs` and
//! `strict.rs`): **refuse only a regular file**. bash's `noclobber_open`
//! (`redir.c`) stats the target first — a regular file is the refusal; nothing
//! there opens **exclusively** (the create stays atomic); an existing
//! non-regular target opens with the direction's flags, `O_EXCL` off.
//!
//! The kernel is not lenient here (measured 2026-11-10, bash 5.3.9 and Python
//! `os.open`, same rows): `O_WRONLY|O_CREAT|O_EXCL|O_TRUNC` answers `EEXIST` for
//! a pipe, a character device, a FIFO, a socket and a directory alike, so the
//! refusal cannot live in the open's errno — fdshell must classify the target,
//! exactly like bash. bash's measured rows for a `set -C` write (`e` below):
//! regular file and a symlink to one (stat **follows** the symlink) refuse;
//! `/dev/null`, a pipe via `/dev/fd/N` and a nonexistent target succeed; a FIFO
//! open blocks without a reader; a directory answers `Is a directory`; a socket
//! answers `No such device or address`. `O_TRUNC` drops on the plain arm, which
//! is invisible: the kernel ignores it for every non-regular type, and every
//! regular case is refused before the open.
//!
//! The strict route's stat follows symlinks out of the pinned `%CWD` root
//! (`statx` flags `0`), matching the `openat2` it feeds (`resolve: 0`, no
//! `RESOLVE_*` constraints today) — one consistent pair, like bash's `stat()`.

// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

use sys::stat::{S_IFMT, S_IFREG};
use sys::{AtFd, ExportedCStr, LocalFd};

/// What bash's stat gate makes of the target: the refusal, the plain open with
/// the direction's own flags, or the exclusive create (bash's "the file was not
/// present" arm — a failed stat lands there, so a create that fails with an
/// errno other than `EEXIST` still propagates its own errno).
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub(super) enum Decision {
    Refuse,
    Plain,
    Exclusive,
}

/// The decision on the process-CWD route: bash's `stat()` on the path, which
/// follows symlinks.
pub(super) fn decide_from_path(name: &ExportedCStr) -> Decision {
    decide(sys::stat::stat(name).ok().map(|st| st.mode))
}

/// The decision on the strict route: `statx` through the pinned `%CWD` fd,
/// flags `0` = follow, like bash's `stat()`.
pub(super) fn decide_from_base(base: &LocalFd, name: &ExportedCStr) -> Decision {
    decide(
        sys::statx::statx(AtFd::from(base), name.as_ref(), 0)
            .ok()
            .map(|st| st.mode),
    )
}

/// One rule, both routes: a **regular** mode is the refusal, any other mode the
/// plain arm, no mode at all (failed stat) the exclusive create (§4.12).
fn decide(mode: Option<u32>) -> Decision {
    match mode {
        Some(mode) if mode & S_IFMT == S_IFREG => Decision::Refuse,
        Some(_) => Decision::Plain,
        None => Decision::Exclusive,
    }
}

#[cfg(test)]
mod tests;

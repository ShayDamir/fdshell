// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

//! The open pass's per-def outcome for the resolver (task #479): the real
//! pre-open fd plus the decision of whether the **final expanded name** is the
//! `/dev/fd/N` magic form, made once by the open pass (`redirect/open/target.rs`)
//! after the target pipeline's fold, strict ban, glob and one-name rule.

use sys::LocalFd;

/// One path def's outcome, as the open pass hands it to the resolver
/// (`redirect/resolve.rs`). The `fd` is **always** the real pre-open — its
/// success is judged by the pre-open pass against the pre-command fds (the
/// boundary that keeps divergence #482 alive), the fd stays owned by the pass
/// and is dropped after resolve. `magic` is `Some((prefix, n))` when the final
/// expanded name is the magic form, and the resolver then runs the table-based
/// open (`redirect/resolve/fd_path.rs`) instead of adopting this fd; the
/// flag applies uniformly to literal, quoted, escape-folded, globbed and
/// expanded words.
pub struct OpenedTarget {
    /// The pass's own pre-open of the final name, owned by the pass.
    pub fd: LocalFd,
    /// The magic reading of the final expanded name: prefix and fd number.
    pub magic: Option<(&'static str, i32)>,
}

impl OpenedTarget {
    /// The per-def outcome the open pass produces for one path def.
    pub fn new(fd: LocalFd, magic: Option<(&'static str, i32)>) -> Self {
        Self { fd, magic }
    }
}

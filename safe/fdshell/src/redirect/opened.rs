// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

//! The open pass's per-def outcome for the resolver (task #479): the pass's own
//! pre-open plus the decision of whether the **final expanded name** is the
//! `/dev/fd/N` magic form, made once by the open pass (`redirect/open/target.rs`)
//! after the target pipeline's fold, strict ban, glob and one-name rule.

use sys::LocalFd;

/// One path def's outcome, as the open pass hands it to the resolver
/// (`redirect/resolve.rs`). `fd` is the pass's own pre-open: `None` for a magic
/// word, whose open belongs to the resolver (`redirect/resolve/fd_path.rs`,
/// judged at its own point in the walk — task #482); `Some` for an ordinary
/// name and for the noclobber gate's exclusive create (its errno *is* the
/// decision). A `Some` fd stays owned by the pass and is dropped after resolve.
/// `magic` is `Some((prefix, n))` when the final expanded name is the magic
/// form; the flag applies uniformly to literal, quoted, escape-folded, globbed
/// and expanded words.
pub struct OpenedTarget {
    /// The pass's own pre-open of the final name, owned by the pass; `None`
    /// when the def's open belongs to the resolver.
    pub fd: Option<LocalFd>,
    /// The magic reading of the final expanded name: prefix and fd number.
    pub magic: Option<(&'static str, i32)>,
}

impl OpenedTarget {
    /// The per-def outcome the open pass produces for one path def.
    pub fn new(fd: Option<LocalFd>, magic: Option<(&'static str, i32)>) -> Self {
        Self { fd, magic }
    }
}

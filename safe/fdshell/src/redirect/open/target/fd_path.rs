//! The magic-path reading of a resolved redirect target name: the prefixes
//! Linux installs and the fd number its digits must name — the reading
//! `redirect/resolve/fd_path.rs`'s open pass consumes (tasks #479/#482).

// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

use sys::ShortCStr;

/// The magic-path prefixes Linux installs, in the order bash tests them.
pub(crate) const PREFIXES: &[&str] = &["/dev/fd/", "/proc/self/fd/"];

/// The magic reading of a resolved target **name**: the prefix it uses and the
/// fd number its digits name, `None` for every other name. Evaluated on the
/// final name after the pipeline in `target.rs`, so the reading covers literal,
/// quoted, escape-folded, globbed and expanded words alike (task #479) — the
/// word bash tokenizes before its redirection step. The digits must be the
/// canonical decimal of the number — all digits, no sign, no leading zero, and a
/// value that fits an `i32` — because the kernel's entry for fd 1 is named `1`:
/// `/dev/fd/01`, `/dev/fd/1x`, `/dev/fd/-1` and `/dev/fd/` are plain paths, and
/// bash fails each of them on the open (rc 1, `No such file or directory`).
/// A flagged def carries no plain-arm pre-open: its `open()` runs at the
/// resolver's point in the walk, judged against the fd state the preceding defs
/// leave (`redirect/resolve/fd_path.rs`, task #482); the pass keeps only the
/// noclobber gate's arms on the expanded word.
pub(crate) fn magic(name: &ShortCStr) -> Option<(&'static str, i32)> {
    PREFIXES.iter().find_map(|p| digits(name, p))
}

/// The `(prefix, number)` of `name` when its tail is the canonical decimal of a
/// fd number under `prefix`.
fn digits(name: &ShortCStr, prefix: &'static str) -> Option<(&'static str, i32)> {
    let tail = name.strip_prefix(prefix.as_bytes())?;
    let bytes = tail.as_bytes().ok()?;
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    // `/dev/fd/0` is fd 0; `/dev/fd/01` is a path that does not exist.
    if bytes.first() == Some(&b'0') && bytes.len() > 1 {
        return None;
    }
    let n = core::str::from_utf8(bytes).ok()?.parse::<i32>().ok()?;
    Some((prefix, n))
}

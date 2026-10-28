//! The `/dev/fd/<n>` and `/proc/self/fd/<n>` redirect targets: Linux's magic
//! paths. bash opens them with the operator's flags **as the redirection runs**,
//! so the open sees the fd state the preceding redirections left
//! (`redirect/resolve/table.rs`) and yields a **fresh** open file description of
//! the target file — its own offset, `>` truncating it. Measured against bash
//! 5.3.9: `f > z 2> /dev/fd/1` leaves `z` = `e\n\n` (two descriptions racing at
//! 0), not the `hi\ne\n` a `dup` shares, and `cat < /dev/fd/0` after a partial
//! read starts at byte 0 (task #337).

use super::{arms, table};
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{Redirect, RedirectDef};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

/// The magic-path prefixes Linux installs, in the order bash tests them.
const PREFIXES: &[&str] = &["/dev/fd/", "/proc/self/fd/"];

/// A magic-path target word: the prefix it uses and the fd number its digits
/// name, `None` for every other word. Recognized on the word **as the parser
/// wrote it** (so `/dev/fd/$n` is an ordinary path: its open is the consumer's,
/// against the pre-command descriptor). The digits must be the canonical decimal
/// of the number — all digits, no sign, no leading zero, and a value that fits
/// an `i32` — because the kernel's entry for fd 1 is named `1`: `/dev/fd/01`,
/// `/dev/fd/1x`, `/dev/fd/-1` and `/dev/fd/` are plain paths, and bash fails
/// each of them on the open (rc 1, `No such file or directory`).
pub(super) fn magic(word: &ShortCStr) -> Option<(&'static str, i32)> {
    PREFIXES.iter().find_map(|p| digits(word, p))
}

/// The `(prefix, number)` of `word` when its tail is the canonical decimal of a
/// fd number under `prefix`.
fn digits(word: &ShortCStr, prefix: &'static str) -> Option<(&'static str, i32)> {
    let tail = word.strip_prefix(prefix.as_bytes())?;
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

/// Resolve a magic-path def against the fd state the preceding defs leave: the
/// number the table resolves for `n` — a preceding def's own fd, else `n`
/// itself — opened under `prefix` with the def's flags. A preceding `N>&-`
/// leaves no fd and the kernel has no entry for a closed one, so the def fails
/// on the open, as bash's does.
pub(super) fn resolve(
    def: &RedirectDef,
    prefix: &str,
    n: i32,
    min_fd: i32,
    built: &[Redirect],
    seed: &[Redirect],
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let raw = table::number(n, built, seed).ok_or_else(|| Report::new(OpenRedirectError::Open))?;
    let word = sys::format!("{prefix}{raw}").change_context(OpenRedirectError::Open)?;
    let src = sys::openat2::open(word.export(), def.direction.open_flags())
        .change_context(OpenRedirectError::Open)?;
    let local = src
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    let pair = arms::pair_of(&src, def.extra_to, min_fd)?;
    Ok((Redirect::new(def.export_to, local), pair))
}

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::origin::Origin;
use crate::error::resolve::ResolveError;

/// Split a word on the shell's `IFS` characters, cutting **only expansion
/// bytes** (POSIX 2.6.5, task #222): a byte delimits a field only when it is
/// in `IFS`, its mask bit is `false`, and `origin` marks it as produced by an
/// expansion. The word's literal bytes are always content, so a literal `IFS`
/// byte glues to the expansion it sits next to (`IFS=,; v="b c"; printf %s
/// x$v,y` is one field `xb c,y`).
///
/// Runs of `IFS` whitespace collapse to one delimiter and trim **the ends of
/// the whole expansion** (not per part); a non-whitespace delimiter merges
/// with a whitespace run that follows it, and a delimiter at the very end of
/// the expansion produces no trailing empty field. An empty `IFS` disables
/// splitting. An `IFS` char whose mask bit is `true` was inside quotes
/// (double **or** single) and never delimits.
/// Each field comes back with the per-byte quote mask of the same span,
/// so later pattern detection can skip quoted bytes.
///
/// A byte index marked in `origin` as a `$@` positional boundary closes the
/// open field: each positional glued into a larger word starts a new field
/// (`IFS=,; set -- "a b" c; printf %s x"$@"y` → `[xa b][cy]`). A boundary
/// flushes the pending empty field produced by a delimiter that just closed,
/// flushes a leading empty field at index 0, and skips the flush when the
/// preceding text was a whitespace run just consumed. A boundary at the very
/// end of the word flushes the last field, and a **quoted** positional there
/// keeps its (empty) field — an unquoted empty positional vanishes.
pub(crate) fn split_word(
    word: &ShortCStr,
    mask: &[bool],
    ifs: &ShortCStr,
    origin: &Origin,
) -> Result<Vec<(ShortCStr, Vec<bool>)>, Report<ResolveError>> {
    let bytes = word.as_bytes().change_context(ResolveError::RefNotFound)?;
    let ifs_bytes = ifs.as_bytes().change_context(ResolveError::RefNotFound)?;
    let quoted = |i: usize| mask.get(i).copied().unwrap_or(false);
    let is_ws = |b: u8| (b == b' ' || b == b'\t' || b == b'\n') && ifs_bytes.contains(&b);
    let is_ifs = |b: u8| ifs_bytes.contains(&b);
    let free = |i: usize| origin.splittable(i) && !quoted(i);
    let mut fields = Vec::new();
    let mut cur = ShortCStr::new();
    let mut cur_mask = Vec::new();
    let mut trailing = false;
    // Flushes the open field like a non-whitespace delimiter does. The `i ==
    // 0` arm yields the leading empty field of a positional that starts the
    // word (`IFS=,; set -- "" b; printf %s $@y` → `[][by]`).
    macro_rules! boundary_flush {
        ($i:expr) => {
            for &(_, _) in origin.boundaries_at($i) {
                if !cur.is_empty() || trailing || $i == 0 {
                    fields.push((core::mem::take(&mut cur), core::mem::take(&mut cur_mask)));
                    trailing = true;
                }
            }
        };
    }
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        boundary_flush!(i);
        if is_ws(b) && free(i) {
            // `trailing` marks a non-whitespace delimiter at the end of the
            // field just closed: it merges with this whitespace run, so the
            // field is not flushed and the delimiter is trimmed with the run
            // (`IFS=" ,": "a, "` is one field `a`).
            if !cur.is_empty() && !trailing {
                fields.push((core::mem::take(&mut cur), core::mem::take(&mut cur_mask)));
            }
            trailing = false;
            // The run consumes unquoted `IFS` whitespace only. No `$@`
            // boundary can cut it: boundaries exist only for a **quoted**
            // `$@`, whose positional bytes inherit the trigger byte's quoted
            // mask bit, so `quoted(i + 1)` already stops the run at the
            // positional's first byte — and a byte the run consumed could
            // never flush anyway (inside a run `cur` is empty, `trailing` is
            // false and `i != 0`), so a boundary there has no observable
            // effect.
            while let Some(&n) = bytes.get(i + 1) {
                if !is_ws(n) || quoted(i + 1) {
                    break;
                }
                i += 1;
            }
        } else if is_ifs(b) && free(i) {
            fields.push((core::mem::take(&mut cur), core::mem::take(&mut cur_mask)));
            trailing = true;
        } else {
            cur.push_byte(b).change_context(ResolveError::NulByte)?;
            cur_mask.push(quoted(i));
            trailing = false;
        }
        i += 1;
    }
    // Positional boundaries at the very end of the word flush the last field
    // (`IFS=,; set -- a b; printf %s x$@` → `[xa][b]`); the final flush below
    // keeps the last positional's own field, empty only when quoted.
    boundary_flush!(bytes.len());
    if !cur.is_empty() || origin.ends_quoted(bytes.len()) {
        fields.push((cur, cur_mask));
    }
    Ok(fields)
}

#[cfg(test)]
mod tests;

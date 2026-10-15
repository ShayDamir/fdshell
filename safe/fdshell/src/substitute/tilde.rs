//! Leading-`~` expansion — `$HOME` when a tilde starts a word.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::mask::{push_byte, push_expanded};
use crate::error::resolve::ResolveError;

/// Whether the word opens a tilde expansion: a leading `~` whose mask bit is
/// **unquoted**. POSIX expands tildes before quote removal, and bash keeps a
/// quoted `~` literal (`"~"`, `"~/x"`, and every here-doc body `~`, whose
/// whole body is a quoted context — task #269), so the mask's first bit gates
/// the arm. A mask shorter than the word pads as unquoted, like `arg.rs`.
pub(super) fn leads(bytes: &[u8], mask: &[bool]) -> bool {
    bytes.first() == Some(&b'~') && !mask.first().copied().unwrap_or(false)
}

/// Expand the leading `~` already at the front of `peek`. `~` alone or
/// `~/…` becomes `$HOME` (nothing when unset); any other `~x` keeps the
/// tilde. Consumed input and appended output keep the quote mask aligned.
/// `expanded` reports that `$HOME` was pushed: an unset `HOME` or a
/// literal `~user` performed no expansion.
pub(super) fn expand(
    peek: &mut super::mask::Counting<'_>,
    idx: &mut usize,
    mask: &[bool],
    expanded: &mut bool,
    out: &mut ShortCStr,
    out_mask: &mut Vec<bool>,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    *idx += 1;
    let home_q = mask.first().copied().unwrap_or(false);
    match peek.peek() {
        None | Some(&b'/') => {
            if let Some(home) = sys::env::getenv(c"HOME") {
                let bytes = home.as_bytes().change_context(ResolveError::RefNotFound)?;
                push_expanded(out, out_mask, bytes, home_q)?;
                *expanded = true;
            }
        }
        _ => push_byte(out, out_mask, b'~', home_q)?,
    }
    Ok(())
}

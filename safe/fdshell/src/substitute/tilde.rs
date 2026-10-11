//! Leading-`~` expansion — `$HOME` when a tilde starts a word.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::mask::{push_byte, push_expanded};
use crate::error::resolve::ResolveError;

/// Expand the leading `~` already at the front of `peek`. `~` alone or
/// `~/…` becomes `$HOME` (nothing when unset); any other `~x` keeps the
/// tilde. Consumed input and appended output keep the quote mask aligned.
/// `expanded` reports that `$HOME` was pushed: an unset `HOME` or a
/// literal `~user` performed no expansion.
pub(super) fn expand(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
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

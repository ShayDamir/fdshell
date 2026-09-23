use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

use super::borrow_state;
use super::split::split_word;

/// Expands a word that is exactly `$@` or `$*` (unlike `$var`, the two differ).
///
/// `"$@"` yields one word per positional; `"$*"` yields one word joined by the
/// first IFS byte (nothing if IFS is empty). Unquoted, each positional is
/// word-split on IFS separately, so a custom IFS cannot leave injected
/// separators behind. Each result word comes back with the per-byte quote
/// mask (all quoted for a quoted expansion, all unquoted otherwise) so
/// later pattern detection can skip quoted bytes.
pub(super) fn expand_positional_word(
    is_star: bool,
    fq: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<(ShortCStr, Vec<bool>)>, Report<ResolveError>> {
    let state = borrow_state(cell)?;
    if fq && is_star {
        let sep = first_ifs_byte(&state.ifs)?;
        let word = join(&state.positional, sep)?;
        let mask = vec![true; word.len()];
        return Ok(vec![(word, mask)]);
    }
    let mut out = Vec::new();
    for p in &state.positional {
        if fq {
            out.push((p.value.clone(), vec![true; p.value.len()]));
        } else {
            for (field, mask) in split_word(&p.value, &[], &state.ifs)? {
                out.push((field, vec![false; mask.len()]));
            }
        }
    }
    Ok(out)
}

/// Joins positional parameters with the first IFS byte, or a space if IFS is
/// empty; used for `$@`/`$*` embedded in a larger unquoted word.
pub(super) fn positional_join<'a>(
    positional: impl IntoIterator<Item = &'a ImportedStr>,
    ifs: &ShortCStr,
) -> Result<ShortCStr, Report<ResolveError>> {
    let sep = first_ifs_byte(ifs)?.unwrap_or(b' ');
    join(positional, Some(sep))
}

fn first_ifs_byte(ifs: &ShortCStr) -> Result<Option<u8>, Report<ResolveError>> {
    let bytes = ifs.as_bytes().change_context(ResolveError::RefNotFound)?;
    Ok(bytes.first().copied())
}

fn join<'a>(
    positional: impl IntoIterator<Item = &'a ImportedStr>,
    sep: Option<u8>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut out = ShortCStr::new();
    for (j, p) in positional.into_iter().enumerate() {
        if j > 0
            && let Some(b) = sep
        {
            out.push_byte(b).change_context(ResolveError::NulByte)?;
        }
        out.push(p);
    }
    Ok(out)
}

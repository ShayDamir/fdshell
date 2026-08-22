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
/// separators behind.
pub(super) fn expand_positional_word(
    is_star: bool,
    fq: bool,
    cell: &ForkCell<ShellState>,
    result: &mut Vec<ShortCStr>,
) -> Result<(), Report<ResolveError>> {
    let state = borrow_state(cell)?;
    if fq && is_star {
        let sep = first_ifs_byte(&state.ifs)?;
        result.push(join(&state.positional, sep)?);
        return Ok(());
    }
    for p in &state.positional {
        if fq {
            result.push(p.value.clone());
        } else {
            result.extend(split_word(&p.value, &state.ifs)?);
        }
    }
    Ok(())
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

use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;

use super::borrow_state;
use super::origin::Origin;
use super::split::split_word;

/// Expands a word that is exactly `$@` or `$*` (unlike `$var`, the two differ).
///
/// `"$@"` yields one word per positional; `"$*"` yields one word joined by the
/// first IFS byte (nothing if IFS is empty). Unquoted, each positional is
/// word-split on IFS separately, so a custom IFS cannot leave injected
/// separators behind. Each result word comes back with the per-byte quote
/// mask (all quoted for a quoted expansion, all unquoted otherwise) so
/// later pattern detection can skip quoted bytes.
///
/// This whole-word path and the embedded `$@`/`$*` arm
/// (`dollar/at_star.rs`, task #222) must agree on the per-positional
/// semantics — a positional's own split, empty ones vanishing unquoted —
/// and both spellings are pinned side by side in `tests/positional.rs`.
/// Here each positional's text *is* the expansion, so the splitter gets
/// `Origin::all` (every byte splittable) and no boundaries.
///
/// Empty-deque contract, load-bearing for `substitute_args`' word count
/// (task #219): `fq` + `$@` returns **no words at all** — bash's `"$@"` *is*
/// the positional word list, so an empty deque is no word, and the caller
/// must not synthesize one; `fq` + `$*` returns its single joined word, empty
/// (so one word) when there are none; unquoted, an empty positional
/// contributes nothing.
pub(super) fn expand_positional_word(
    is_star: bool,
    fq: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<(ShortCStr, Vec<MaskBit>)>, Report<ResolveError>> {
    let state = borrow_state(cell)?;
    if fq && is_star {
        let sep = first_ifs_byte(&state.ifs)?;
        let word = join(&state.positional, sep)?;
        let mask = vec![MaskBit::Quoted; word.len()];
        return Ok(vec![(word, mask)]);
    }
    let mut out = Vec::new();
    for p in &state.positional {
        if fq {
            out.push((p.value.clone(), vec![MaskBit::Quoted; p.value.len()]));
        } else {
            for (field, mask) in split_word(&p.value, &[], &state.ifs, &Origin::all(p.value.len()))?
            {
                out.push((field, vec![MaskBit::Unquoted; mask.len()]));
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

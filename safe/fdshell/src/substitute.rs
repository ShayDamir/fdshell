mod arg;
mod backtick;
mod brace;
mod dollar;
mod mask;
mod origin;
mod param_op;
mod paren;
mod percent;
mod positional;
pub(crate) mod resolve;
mod split;
mod subst_paren;
mod tilde;
mod words;

pub use words::substitute_args;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::{ForkCell, Ref};
use sys::{ExportedFd, ShortCStr};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// Substitute one word, discarding the byte provenance: the wrapper for
/// every caller that does not split the result (assignments, heredocs,
/// `local`/`let`, `case` — the value is stored whole, LESSONS). `spans` is
/// the word's zero-byte quote-span list, parallel to the mask (task #321).
pub(crate) fn substitute_arg(
    arg: &ShortCStr,
    mask: &[bool],
    spans: &[usize],
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(ShortCStr, Vec<bool>), Report<ResolveError>> {
    let (out, out_mask, _origin) = arg::substitute_arg_marked(arg, mask, spans, cache, cell)?;
    Ok((out, out_mask))
}

pub(crate) fn borrow_state(
    cell: &ForkCell<ShellState>,
) -> Result<Ref<'_, ShellState>, Report<ResolveError>> {
    cell.borrow().change_context(ResolveError::RefNotFound)
}

/// A word is fully quoted when every byte was consumed inside double quotes,
/// or when the word is empty — an empty mask only arises from quoted
/// material (a quoted empty word like `""`, or a quoted expansion that
/// produced nothing), which is one word even when empty.
pub(super) fn fully_quoted(mask: &[bool]) -> bool {
    mask.is_empty() || mask.iter().all(|&q| q)
}

#[cfg(test)]
mod tests;

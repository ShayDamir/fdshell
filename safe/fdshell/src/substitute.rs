mod arg;
mod brace;
mod dollar;
mod mask;
mod param_op;
mod paren;
mod percent;
mod positional;
pub(crate) mod resolve;
mod split;
mod subst_paren;
mod tilde;
mod words;

pub(crate) use arg::substitute_arg;
pub use words::substitute_args;

use error_stack::{Report, ResultExt};
use sys::fork_cell::{ForkCell, Ref};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

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

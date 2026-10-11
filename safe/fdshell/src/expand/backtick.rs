//! The whole-word backtick arm of a `for … in` list.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::cmd_subst;
use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(super) use super::fields::{split_whitespace, strip_delims};

/// Run a whole-word backtick body and split its output into words; a quoted
/// word whose output is empty is one empty word.
///
/// Delete this arm when task #216 expands backticks inside `substitute_arg`:
/// it exists only because the argument pipeline leaves a backtick literal
/// (`for w in \`echo hi\`` would iterate over `` `echo` `` and `hi`).
pub(super) fn expand(
    word: &ShortCStr,
    quoted: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let bs = word.as_bytes().change_context(ResolveError::RefNotFound)?;
    let output =
        cmd_subst::run_and_capture(strip_delims(bs), cell).change_context(ResolveError::Resolve)?;
    let mut fields = split_whitespace(&output)?;
    if fields.is_empty() && quoted {
        fields.push(ShortCStr::new());
    }
    Ok(fields)
}

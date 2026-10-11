//! The whole-word backtick arm of a `for … in` list.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::cmd_subst;
use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::borrow_state;

pub(super) use super::fields::strip_delims;

/// Run a whole-word backtick body and split its output into shell words; a
/// quoted word whose output is empty is one empty word.
///
/// The split follows the word's quote mask exactly like every other expansion:
/// a fully quoted command substitution is one word (its spaces, tabs and
/// newlines are data, `IFS` never applies), an unquoted one is split on `IFS`
/// by `split_word`.
///
/// Delete this arm when task #216 expands backticks inside `substitute_arg`:
/// it exists only because the argument pipeline leaves a backtick literal
/// (`for w in \`echo hi\`` would iterate over `` `echo` `` and `hi`).
pub(super) fn expand(
    word: &ShortCStr,
    mask: &[bool],
    quoted: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let bs = word.as_bytes().change_context(ResolveError::RefNotFound)?;
    let output =
        cmd_subst::run_and_capture(strip_delims(bs), cell).change_context(ResolveError::Resolve)?;
    let out = ShortCStr::from_vec(output).change_context(ResolveError::NulByte)?;
    // A quoted substitution is one word; unquoted, its output is split like
    // any other expansion — with an empty mask, since command output carries
    // no quote protection of its own. The `ifs` borrow ends before returning
    // (RefCell, LESSONS.md).
    let mut fields = if crate::substitute::fully_quoted(mask) {
        alloc::vec![out]
    } else {
        let ifs = borrow_state(cell)?.ifs.clone();
        crate::substitute::split_word(&out, &[], &ifs)?
            .into_iter()
            .map(|(field, _)| field)
            .collect::<Vec<ShortCStr>>()
    };
    if fields.is_empty() && quoted {
        fields.push(ShortCStr::new());
    }
    Ok(fields)
}

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
use alloc::vec;
use alloc::vec::Vec;

pub(crate) use arg::substitute_arg;

use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::{ForkCell, Ref};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn borrow_state(
    cell: &ForkCell<ShellState>,
) -> Result<Ref<'_, ShellState>, Report<ResolveError>> {
    cell.borrow().change_context(ResolveError::RefNotFound)
}

pub fn substitute_args(
    args: &[ShortCStr],
    args_mask: &[Vec<bool>],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let mut result = Vec::new();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    for (i, arg) in args.iter().enumerate() {
        let mask = args_mask.get(i).cloned().unwrap_or_default();
        if arg.eq_bytes(b"$@") || arg.eq_bytes(b"$*") {
            for (word, mask) in
                positional::expand_positional_word(arg.eq_bytes(b"$*"), fully_quoted(&mask), cell)?
            {
                push_word(&mut result, &word, &mask, cell)?;
            }
            continue;
        }
        // A fully quoted word is exactly one word — kept even when the
        // expansion is empty (`"${Y:+a}"` → one empty argument).
        let fq = fully_quoted(&mask);
        let (expanded, mask) = arg::substitute_arg(arg, &mask, &mut cache, cell)?;
        // The IFS borrow must end before the glob reborrows the cell for
        // `nullglob` (RefCell, LESSONS.md).
        let fields = if fq {
            vec![(expanded, mask)]
        } else {
            let state = borrow_state(cell)?;
            split::split_word(&expanded, &mask, &state.ifs)?
        };
        for (word, mask) in fields {
            push_word(&mut result, &word, &mask, cell)?;
        }
    }
    Ok(result)
}

/// One word after IFS splitting: a fully quoted word is kept as-is (even
/// empty), any other word is pathname-expanded first.
fn push_word(
    result: &mut Vec<ShortCStr>,
    word: &ShortCStr,
    mask: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<ResolveError>> {
    if fully_quoted(mask) {
        result.push(word.clone());
        return Ok(());
    }
    for expanded in crate::glob::expand(word, mask, cell)? {
        result.push(expanded);
    }
    Ok(())
}

/// A word is fully quoted when every byte was consumed inside double quotes,
/// or when the word is empty — an empty mask only arises from quoted
/// material (a quoted empty word like `""`, or a quoted expansion that
/// produced nothing), which is one word even when empty.
fn fully_quoted(mask: &[bool]) -> bool {
    mask.is_empty() || mask.iter().all(|&q| q)
}

#[cfg(test)]
mod tests;

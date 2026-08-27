mod arg;
mod brace;
mod dollar;
mod mask;
mod param_op;
mod paren;
mod percent;
mod positional;
mod resolve;
mod split;
mod tilde;
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
            positional::expand_positional_word(
                arg.eq_bytes(b"$*"),
                fully_quoted(&mask),
                cell,
                &mut result,
            )?;
        } else {
            // A fully quoted word is exactly one word — kept even when the
            // expansion is empty (`"${Y:+a}"` → one empty argument).
            let fq = fully_quoted(&mask);
            let (expanded, mask) = arg::substitute_arg(arg, &mask, &mut cache, cell)?;
            if fq {
                result.push(expanded);
            } else {
                let state = borrow_state(cell)?;
                result.extend(split::split_word(&expanded, &mask, &state.ifs)?);
            }
        }
    }
    Ok(result)
}

/// A word is fully quoted when every byte was consumed inside double quotes.
fn fully_quoted(mask: &[bool]) -> bool {
    !mask.is_empty() && mask.iter().all(|&q| q)
}

#[cfg(test)]
mod tests;

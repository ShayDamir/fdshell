mod arg;
mod brace;
mod dollar;
mod param_op;
mod paren;
mod percent;
mod positional;
mod resolve;
mod split;
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
    args_fq: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let mut result = Vec::new();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    for (i, arg) in args.iter().enumerate() {
        let fq = args_fq.get(i).copied().unwrap_or(false);
        if arg.eq_bytes(b"$@") || arg.eq_bytes(b"$*") {
            positional::expand_positional_word(arg.eq_bytes(b"$*"), fq, cell, &mut result)?;
        } else {
            let expanded = arg::substitute_arg(arg, &mut cache, cell)?;
            if fq {
                result.push(expanded);
            } else {
                let state = borrow_state(cell)?;
                result.extend(split::split_word(&expanded, &state.ifs)?);
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

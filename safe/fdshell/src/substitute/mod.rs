mod arg;
mod brace;
mod dollar;
mod param_op;
mod paren;
mod percent;
mod resolve;
mod split;
use alloc::vec::Vec;

pub(crate) use arg::substitute_arg;

use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ImportedStr;
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
        if fq && arg.eq_bytes(b"$@") {
            let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
            expand_positional_args(&state.positional, &mut result)?;
        } else if fq && arg.eq_bytes(b"$*") {
            let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
            let expanded = join_positional_args(&state.positional)?;
            result.push(expanded);
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

fn expand_positional_args<'a>(
    positional: impl IntoIterator<Item = &'a ImportedStr>,
    result: &mut Vec<ShortCStr>,
) -> Result<(), Report<ResolveError>> {
    for p in positional {
        result.push(p.value.clone());
    }
    Ok(())
}

fn join_positional_args<'a>(
    positional: impl IntoIterator<Item = &'a ImportedStr>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut out = ShortCStr::new();
    for (j, p) in positional.into_iter().enumerate() {
        if j > 0 {
            out.push(c" ");
        }
        out.push(p);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;

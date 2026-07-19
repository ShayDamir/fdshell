mod arg;
mod brace;
mod dollar;
mod paren;
mod percent;
mod resolve;
use alloc::vec::Vec;

pub(crate) use arg::substitute_arg;

use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub fn substitute_args(
    args: &[ShortCStr],
    args_fq: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let mut result = Vec::new();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
    for (i, arg) in args.iter().enumerate() {
        let fq = args_fq.get(i).copied().unwrap_or(false);
        if fq && arg.eq_bytes(b"$@") {
            expand_positional_args(&state.positional, &mut result)?;
        } else if fq && arg.eq_bytes(b"$*") {
            let expanded = join_positional_args(&state.positional)?;
            result.push(expanded);
        } else {
            let expanded = arg::substitute_arg(arg, &mut cache, cell)?;
            result.push(expanded);
        }
    }
    Ok(result)
}

fn expand_positional_args<'a>(
    positional: impl IntoIterator<Item = &'a ShortCStr>,
    result: &mut Vec<ShortCStr>,
) -> Result<(), Report<ResolveError>> {
    for p in positional {
        result.push(p.clone());
    }
    Ok(())
}

fn join_positional_args<'a>(
    positional: impl IntoIterator<Item = &'a ShortCStr>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut out = ShortCStr::new();
    for (j, p) in positional.into_iter().enumerate() {
        if j > 0 {
            out.push(b' ').change_context(ResolveError::Never)?;
        }
        out.push_str(p).change_context(ResolveError::Never)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;

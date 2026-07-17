mod brace;
use alloc::vec::Vec;
mod dollar;
mod paren;
mod percent;

use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn substitute_arg(
    arg: &ShortCStr,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let bytes = arg.as_bytes().change_context(ResolveError::RefNotFound)?;
    let mut out = ShortCStr::new();
    let mut peek = bytes.iter().copied().peekable();
    if bytes.first() == Some(&b'~') {
        peek.next();
        match peek.peek() {
            None | Some(&b'/') => {
                if let Some(home) = sys::env::getenv(b"HOME") {
                    out.extend_from_slice(&home)
                        .change_context(ResolveError::NulByte)?;
                }
            }
            _ => out.push(b'~').change_context(ResolveError::NulByte)?,
        }
    }
    while let Some(b) = peek.next() {
        match b {
            b'%' => {
                let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
                percent::percent_subst(&mut peek, cache, &state, &mut out)?;
            }
            b'$' if peek.peek() == Some(&b'(') => {
                peek.next();
                let inner = paren::read_paren_expr(&mut peek)?;
                let expanded = crate::cmd_subst::run_and_capture(&inner, cell)
                    .change_context(ResolveError::Resolve)?;
                out.extend_from_slice(&expanded)
                    .change_context(ResolveError::NulByte)?;
            }
            b'$' => {
                let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
                dollar::dollar_subst(&mut peek, &state, &mut out)?;
            }
            _ => out.push(b).change_context(ResolveError::NulByte)?,
        }
    }
    Ok(out)
}

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
        let bytes = arg.as_bytes().change_context(ResolveError::RefNotFound)?;
        if fq && bytes == b"$@" {
            // Quoted "$@": each positional arg becomes a separate token (no further expansion)
            for p in &state.positional {
                result.push(p.clone());
            }
        } else if fq && bytes == b"$*" {
            // Quoted "$*": join positional args with spaces (no further expansion)
            let mut out = ShortCStr::new();
            for (j, p) in state.positional.iter().enumerate() {
                if j > 0 {
                    out.push(b' ').change_context(ResolveError::Never)?;
                }
                out.extend_from_slice(p.as_bytes().change_context(ResolveError::RefNotFound)?)
                    .change_context(ResolveError::NulByte)?;
            }
            result.push(out);
        } else {
            let expanded = substitute_arg(arg, &mut cache, cell)?;
            result.push(expanded);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

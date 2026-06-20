#![forbid(unsafe_code)]

mod dollar;
mod paren;
mod percent;

use error_stack::{Report, ResultExt};
use std::collections::HashMap;
use std::ffi::CString;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn substitute_arg(
    arg: &ShortCStr,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<CString, Report<ResolveError>> {
    let bytes = arg
        .as_bytes()
        .map_err(|_| Report::new(ResolveError::RefNotFound))?;
    let mut out = Vec::new();
    let mut peek = bytes.iter().copied().peekable();
    if bytes.first() == Some(&b'~') {
        peek.next();
        match peek.peek() {
            None | Some(&b'/') => {
                if let Ok(home) = std::env::var("HOME") {
                    out.extend_from_slice(home.as_bytes());
                }
            }
            _ => out.push(b'~'),
        }
    }
    let mut state = cell
        .borrow()
        .map_err(|_| Report::new(ResolveError::RefNotFound))?;
    while let Some(b) = peek.next() {
        match b {
            b'%' => percent::percent_subst(&mut peek, cache, &state, &mut out)?,
            b'$' if peek.peek() == Some(&b'(') => {
                peek.next();
                drop(state);
                let inner = paren::read_paren_expr(&mut peek)?;
                let expanded = crate::cmd_subst::run_and_capture(&inner, cell)
                    .change_context(ResolveError::Resolve)?;
                out.extend_from_slice(&expanded);
                state = cell
                    .borrow()
                    .map_err(|_| Report::new(ResolveError::RefNotFound))?;
            }
            b'$' => dollar::dollar_subst(&mut peek, &state, &mut out)?,
            _ => out.push(b),
        }
    }
    CString::new(out).change_context(ResolveError::NulByte)
}

pub fn substitute_args(
    args: &[ShortCStr],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<CString>, Report<ResolveError>> {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    args.iter()
        .map(|a| substitute_arg(a, &mut cache, cell))
        .collect()
}

#[cfg(test)]
mod tests;

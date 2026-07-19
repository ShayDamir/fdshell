//! Single-argument substitution — handles ~, %, $(), and $.

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
                if let Some(home) = sys::env::getenv(c"HOME") {
                    out.push_str(&home).change_context(ResolveError::Never)?;
                }
            }
            _ => out.push(b'~').change_context(ResolveError::NulByte)?,
        }
    }
    while let Some(b) = peek.next() {
        match b {
            b'%' => {
                let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
                crate::substitute::percent::percent_subst(&mut peek, cache, &state, &mut out)?;
            }
            b'$' if peek.peek() == Some(&b'(') => {
                peek.next();
                let inner = crate::substitute::paren::read_paren_expr(&mut peek)?;
                let expanded = crate::cmd_subst::run_and_capture(&inner, cell)
                    .change_context(ResolveError::Resolve)?;
                out.push_slice(&expanded)
                    .change_context(ResolveError::NulByte)?;
            }
            b'$' => {
                let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
                crate::substitute::dollar::dollar_subst(&mut peek, &state, &mut out)?;
            }
            _ => out.push(b).change_context(ResolveError::NulByte)?,
        }
    }
    Ok(out)
}

use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(crate) fn dollar_subst(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    match peek.peek().copied() {
        Some(b'$') => {
            peek.next();
            let state = super::borrow_state(cell)?;
            core::write!(out, "{}", state.shell_pid).change_context(ResolveError::Never)?;
        }
        Some(b'!') => {
            peek.next();
            let state = super::borrow_state(cell)?;
            if let Some(pid) = state.last_bg_pid {
                core::write!(out, "{pid}").change_context(ResolveError::Never)?;
            }
        }
        Some(b'{') => super::brace::handle_brace(peek, cell, out)?,
        Some(b'#') => {
            peek.next();
            let state = super::borrow_state(cell)?;
            core::write!(out, "{}", state.positional.len()).change_context(ResolveError::Never)?;
        }
        Some(b'@') | Some(b'*') => {
            peek.next();
            let state = super::borrow_state(cell)?;
            let joined = super::positional::positional_join(&state.positional, &state.ifs)?;
            out.push(&joined);
        }
        Some(c @ b'0'..=b'9') => {
            // $0, $1, ... $N
            peek.next();
            let state = super::borrow_state(cell)?;
            super::resolve::resolve_positional_index(c, peek, &state, out)?;
        }
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = super::percent::collect_name(peek)?;
            let state = super::borrow_state(cell)?;
            if name_scs.eq_bytes(b"_") {
                // `$_`: the `_` variable set by the shell after each command.
                // Empty when unset, unlike ordinary variables (literal `$name`).
                if let Some(val) = super::resolve::var_value(&name_scs, &state) {
                    out.push(val);
                }
            } else {
                super::resolve::resolve_var_name(&name_scs, &state, out)?;
            }
        }
        Some(b'?') => {
            peek.next();
            let state = super::borrow_state(cell)?;
            let code = state.last_status.exit_code();
            core::write!(out, "{code}").change_context(ResolveError::Never)?;
        }
        _ => out.push(c"$"),
    }
    Ok(())
}

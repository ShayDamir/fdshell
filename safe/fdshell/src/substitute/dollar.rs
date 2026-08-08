use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::ShortCStr;

pub(crate) fn dollar_subst(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    match peek.peek().copied() {
        Some(b'$') => {
            peek.next();
            core::write!(out, "{}", state.shell_pid).change_context(ResolveError::Never)?;
        }
        Some(b'!') => {
            peek.next();
            if let Some(pid) = state.last_bg_pid {
                core::write!(out, "{pid}").change_context(ResolveError::Never)?;
            }
        }
        Some(b'{') => super::brace::handle_brace(peek, state, out)?,
        Some(b'#') => {
            peek.next();
            core::write!(out, "{}", state.positional.len()).change_context(ResolveError::Never)?;
        }
        Some(b'@') | Some(b'*') => {
            peek.next();
            join_positional(out, state)?;
        }
        Some(c @ b'0'..=b'9') => {
            // $0, $1, ... $N
            peek.next();
            super::resolve::resolve_positional_index(c, peek, state, out)?;
        }
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = super::percent::collect_name(peek)?;
            super::resolve::resolve_var_name(&name_scs, state, out)?;
        }
        Some(b'?') => {
            peek.next();
            let code = state.last_status.exit_code();
            core::write!(out, "{code}").change_context(ResolveError::Never)?;
        }
        _ => out.push_cstr(c"$"),
    }
    Ok(())
}

fn join_positional(out: &mut ShortCStr, state: &ShellState) -> Result<(), Report<ResolveError>> {
    for (i, p) in state.positional.iter().enumerate() {
        if i > 0 {
            out.push_cstr(c" ");
        }
        out.push_str(p);
    }
    Ok(())
}

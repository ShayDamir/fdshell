mod at_star;

use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::origin::Origin;

/// The `$` arm's quote-region context: the output mask, the word index of the
/// byte after `$` (the `${…}` arm reads the mask under its own content
/// bytes), and the name cap of the trigger byte's quote region.
pub(crate) struct Region<'a> {
    pub mask: &'a [MaskBit],
    pub start: usize,
    pub limit: usize,
}

pub(crate) fn dollar_subst(
    peek: &mut super::mask::Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    region: Region<'_>,
    quoted: bool,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    let Region { mask, start, limit } = region;
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
        Some(b'{') => super::brace::handle_brace(peek, mask, start, cell, out)?,
        Some(b'#') => {
            peek.next();
            let state = super::borrow_state(cell)?;
            core::write!(out, "{}", state.positional.len()).change_context(ResolveError::Never)?;
        }
        // `$@`/`$*` inside a larger word: the arm lives in `dollar/at_star.rs`.
        Some(b'@') | Some(b'*') => at_star::expand(peek, cell, out, quoted, origin)?,
        Some(c @ b'0'..=b'9') => {
            // `$0` is the program name; `$1, $2, ... $N` are the positionals.
            peek.next();
            let state = super::borrow_state(cell)?;
            super::resolve::resolve_positional_index(c, peek, &state, out, limit)?;
        }
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            // A name never crosses out of its trigger byte's quote region
            // (`"$v"bc` is the parameter `v` plus the literal `bc`), so a
            // region that ends at the `$` leaves no name and the `$` is
            // literal — the same rule `$'…'` and `$"…"` already follow.
            if limit == 0 {
                out.push(c"$");
                return Ok(());
            }
            let name_scs = super::percent::collect_name(peek, limit)?;
            let state = super::borrow_state(cell)?;
            if name_scs.eq_bytes(b"_") {
                // `$_`: the `_` variable set by the shell after each command.
                // Empty when unset, like every other parameter; `$_` is not
                // nounset-checked (accepted divergence: bash never reports `_`
                // unbound in `-c` mode, because `set -u` itself binds it).
                if let Some(val) = state.var_value(&name_scs) {
                    out.push(val);
                }
            } else {
                state.resolve_var_name(&name_scs, out)?;
            }
        }
        Some(b'?') => {
            peek.next();
            let state = super::borrow_state(cell)?;
            let code = state.last_status.exit_code();
            core::write!(out, "{code}").change_context(ResolveError::Never)?;
        }
        Some(b'-') => {
            // `$-`: the active shell options' short flags (bash compat).
            peek.next();
            let state = super::borrow_state(cell)?;
            out.push_checked(&crate::options::flags(state.options))
                .change_context(ResolveError::NulByte)?;
        }
        _ => out.push(c"$"),
    }
    Ok(())
}

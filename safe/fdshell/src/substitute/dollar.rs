mod at_star;
mod name;

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
        // The two region-capped arms — `$<digit…>` and `$<name>` — live in
        // `dollar/name.rs`: the region cap is checked there **before** the
        // trigger byte is consumed, so the borrow of the state stays below it.
        Some(c @ b'0'..=b'9') => name::positional(c, peek, cell, out, limit)?,
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            name::variable(peek, cell, out, limit)?
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

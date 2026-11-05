//! The `${#…}` arm of brace expansion: length of a variable, element count of
//! an array, or length of an element — task #422, moved out of `brace.rs`.

use core::fmt::Write;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

use crate::substitute::brace::read_until_close;
use crate::substitute::mask::Counting;
use crate::substitute::{array_fields, borrow_state};

pub(crate) fn length(
    peek: &mut Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    let state = borrow_state(cell)?;
    let (name, closed) = read_until_close(peek)?;
    // `${#}` is the length of the empty name: bash prints `0` at rc 0 even
    // under `set -u`, so this case must not reach the nounset bail (an
    // `UnboundVariable` with an empty name names nothing, STYLE §4.7).
    if closed && name.is_empty() {
        out.push(c"0");
        return Ok(());
    }
    if closed {
        // The array arm of `#`: the element count (`${#name[@]}`) and the
        // element's length (`${#name[idx]}`) — task #422.
        if let Some(len) = array_fields::len(&name, cell)? {
            core::write!(out, "{len}").change_context(ResolveError::Never)?;
            return Ok(());
        }
    }
    match (closed, state.var_value(&name).map(|v| v.len())) {
        (true, Some(len)) => {
            core::write!(out, "{len}").change_context(ResolveError::Never)?;
        }
        // nounset bails on `${#name}` like bash's `x: unbound variable`.
        (true, None) if state.options & crate::options::NOUNSET != 0 => {
            bail!(ResolveError::UnboundVariable { var: name });
        }
        // POSIX 2.7.1: an unset parameter has length 0.
        (true, None) => out.push(c"0"),
        (false, _) => {
            out.push(c"${#");
            out.push(&name);
        }
    }
    Ok(())
}

use core::fmt::Write;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn handle_brace(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    if peek.peek().copied() == Some(b'#') {
        peek.next();
        let state = super::borrow_state(cell)?;
        let (name, closed) = read_until_close(peek)?;
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
        return Ok(());
    }
    let (content, closed) = read_until_close(peek)?;
    if !closed {
        out.push(c"${");
        out.push(&content);
        return Ok(());
    }
    if let Some((name, op, word)) = super::param_op::split_operator(&content) {
        return super::param_op::apply_param_op(&name, op, &word, cell, out);
    }
    let state = super::borrow_state(cell)?;
    // An empty braced name stays literal: bash rejects `${}` (`bad
    // substitution`, rc 1) and expands `${!}` (an empty indirect name) to
    // empty at rc 0; fdshell prints both literally — a documented divergence.
    let indirect = content.strip_prefix(b"!");
    if content.is_empty() || indirect.as_ref().is_some_and(|name| name.is_empty()) {
        out.push(c"${");
        out.push(&content);
        out.push(c"}");
        return Ok(());
    }
    if indirect.is_some() {
        return state.resolve_indirect(&content, out);
    }
    state.resolve_var_name(&content, out)
}

fn read_until_close(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<(ShortCStr, bool), Report<ResolveError>> {
    let mut name = ShortCStr::new();
    let mut closed = false;
    for nc in peek.by_ref() {
        if nc == b'}' {
            closed = true;
            break;
        }
        name.push_byte(nc).change_context(ResolveError::NulByte)?;
    }
    Ok((name, closed))
}

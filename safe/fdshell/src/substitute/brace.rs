use core::fmt::Write;
use error_stack::{Report, ResultExt};
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
        match (
            closed,
            super::resolve::var_value(&name, &state).map(|v| v.len()),
        ) {
            (true, Some(len)) => {
                core::write!(out, "{len}").change_context(ResolveError::Never)?;
            }
            (true, None) => {
                out.push(c"${#");
                out.push(&name);
                out.push(c"}");
            }
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
    if let Some(name) = content.strip_prefix(b"!") {
        super::resolve::resolve_indirect(&name, &state, out);
        return Ok(());
    }
    match super::resolve::var_value(&content, &state) {
        Some(val) => out.push(val),
        None => super::resolve::literal_braced(false, &content, out),
    }
    Ok(())
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

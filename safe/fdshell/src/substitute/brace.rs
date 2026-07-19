use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn handle_brace(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    if peek.peek().copied() == Some(b'#') {
        peek.next();
        let (name, closed) = read_until_close(peek)?;
        if closed {
            if let Some(val) = state.strings.get(&name) {
                core::write!(out, "{}", val.len()).change_context(ResolveError::Never)?;
            } else {
                out.push_slice(b"${#")
                    .and_then(|_| out.push_str(&name))
                    .and_then(|_| out.push(b'}'))
                    .change_context(ResolveError::Never)?;
            }
        } else {
            out.push_slice(b"${#")
                .and_then(|_| out.push_str(&name))
                .change_context(ResolveError::Never)?;
        }
        return Ok(());
    }
    let (name, closed) = read_until_close(peek)?;
    if closed {
        match state.strings.get(&name) {
            Some(val) => out.push_str(val).change_context(ResolveError::Never)?,
            None => {
                out.push_slice(b"${")
                    .and_then(|_| out.push_str(&name))
                    .and_then(|_| out.push(b'}'))
                    .change_context(ResolveError::Never)?;
            }
        }
    } else {
        out.push_slice(b"${")
            .and_then(|_| out.push_str(&name))
            .change_context(ResolveError::Never)?;
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
        name.push(nc).change_context(ResolveError::NulByte)?;
    }
    Ok((name, closed))
}

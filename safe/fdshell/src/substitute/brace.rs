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
            let len = state.strings.get(&name).map(|val| val.len()).or_else(|| {
                state
                    .environ
                    .iter()
                    .find(|(k, _)| k == &name)
                    .map(|(_, v)| v.len())
            });
            match len {
                Some(len) => core::write!(out, "{}", len).change_context(ResolveError::Never)?,
                None => {
                    out.push(c"${#");
                    out.push(&name);
                    out.push(c"}");
                }
            }
        } else {
            out.push(c"${#");
            out.push(&name);
        }
        return Ok(());
    }
    let (name, closed) = read_until_close(peek)?;
    if closed {
        match state.strings.get(&name) {
            Some(val) => out.push(val),
            None => match state.environ.iter().find(|(k, _)| k == &name) {
                Some((_, val)) => out.push(val),
                None => {
                    out.push(c"${");
                    out.push(&name);
                    out.push(c"}");
                }
            },
        }
    } else {
        out.push(c"${");
        out.push(&name);
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

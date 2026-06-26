#![forbid(unsafe_code)]

use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn handle_brace(
    peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    state: &ShellState,
    out: &mut Vec<u8>,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    if peek.peek().copied() == Some(b'#') {
        peek.next();
        let (name, closed) = read_until_close(peek);
        if closed {
            let name_scs = ShortCStr::from_vec(name).change_context(ResolveError::NulByte)?;
            if let Some(val) = state.strings.get(&name_scs) {
                let len = val
                    .as_bytes()
                    .change_context(ResolveError::RefNotFound)?
                    .len();
                let s = format!("{}", len);
                out.extend_from_slice(s.as_bytes());
            } else {
                out.push(b'$');
                out.push(b'{');
                out.push(b'#');
                out.extend_from_slice(
                    name_scs
                        .as_bytes()
                        .change_context(ResolveError::RefNotFound)?,
                );
                out.push(b'}');
            }
        } else {
            out.push(b'$');
            out.push(b'{');
            out.push(b'#');
            out.extend_from_slice(&name);
        }
        return Ok(());
    }
    let (name, closed) = read_until_close(peek);
    if closed {
        let name_scs = ShortCStr::from_vec(name).change_context(ResolveError::NulByte)?;
        match state.strings.get(&name_scs) {
            Some(val) => {
                out.extend_from_slice(val.as_bytes().change_context(ResolveError::RefNotFound)?)
            }
            None => {
                out.push(b'$');
                out.push(b'{');
                out.extend_from_slice(
                    name_scs
                        .as_bytes()
                        .change_context(ResolveError::RefNotFound)?,
                );
                out.push(b'}');
            }
        }
    } else {
        out.push(b'$');
        out.push(b'{');
        out.extend_from_slice(&name);
    }
    Ok(())
}

fn read_until_close(peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>) -> (Vec<u8>, bool) {
    let mut name = Vec::new();
    let mut closed = false;
    while let Some(&nc) = peek.peek() {
        if nc == b'}' {
            closed = true;
            peek.next();
            break;
        }
        name.push(nc);
        peek.next();
    }
    (name, closed)
}

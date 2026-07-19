use alloc::vec::Vec;
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
        let (name, closed) = read_until_close(peek);
        if closed {
            let name_scs = ShortCStr::from_vec(name).change_context(ResolveError::NulByte)?;
            if let Some(val) = state.strings.get(&name_scs) {
                let len = val
                    .as_bytes()
                    .change_context(ResolveError::RefNotFound)?
                    .len();
                core::write!(out, "{len}").change_context(ResolveError::NulByte)?;
            } else {
                out.extend_from_slice(b"${#")
                    .change_context(ResolveError::Never)?;
                out.extend_from_slice(
                    name_scs
                        .as_bytes()
                        .change_context(ResolveError::RefNotFound)?,
                )
                .change_context(ResolveError::NulByte)?;
                out.push(b'}').change_context(ResolveError::Never)?;
            }
        } else {
            out.extend_from_slice(b"${#")
                .change_context(ResolveError::Never)?;
            for &b in &name {
                out.push(b).change_context(ResolveError::Never)?;
            }
        }
        return Ok(());
    }
    let (name, closed) = read_until_close(peek);
    if closed {
        let name_scs = ShortCStr::from_vec(name).change_context(ResolveError::NulByte)?;
        match state.strings.get(&name_scs) {
            Some(val) => out
                .extend_from_slice(val.as_bytes().change_context(ResolveError::RefNotFound)?)
                .change_context(ResolveError::NulByte)?,
            None => {
                out.extend_from_slice(b"${")
                    .change_context(ResolveError::Never)?;
                out.extend_from_slice(
                    name_scs
                        .as_bytes()
                        .change_context(ResolveError::RefNotFound)?,
                )
                .change_context(ResolveError::NulByte)?;
                out.push(b'}').change_context(ResolveError::Never)?;
            }
        }
    } else {
        out.extend_from_slice(b"${")
            .change_context(ResolveError::Never)?;
        for &b in &name {
            out.push(b).change_context(ResolveError::Never)?;
        }
    }
    Ok(())
}

fn read_until_close(peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>) -> (Vec<u8>, bool) {
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

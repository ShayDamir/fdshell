#![forbid(unsafe_code)]

use error_stack::{Report, ResultExt};
use std::collections::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn collect_name(
    peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut name = Vec::new();
    name.push(peek.next().ok_or(ResolveError::RefNotFound)?);
    while let Some(&nc) = peek.peek() {
        if nc.is_ascii_alphanumeric() || nc == b'_' {
            name.push(nc);
            peek.next();
        } else {
            break;
        }
    }
    ShortCStr::from_vec(name).change_context(ResolveError::NulByte)
}

pub(crate) fn percent_subst(
    peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    state: &ShellState,
    out: &mut Vec<u8>,
) -> Result<(), Report<ResolveError>> {
    match peek.peek().copied() {
        Some(b'%') => {
            out.push(b'%');
            peek.next();
        }
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = collect_name(peek)?;
            let raw = match cache.get(&name_scs) {
                Some(d) => d.as_raw(),
                None => match state.fds.get(&name_scs) {
                    Some(src) => {
                        let d = src.export().change_context(ResolveError::RefNotFound)?;
                        let raw = d.as_raw();
                        cache.insert(name_scs, d);
                        raw
                    }
                    None => {
                        out.push(b'%');
                        out.extend_from_slice(
                            name_scs
                                .as_bytes()
                                .change_context(ResolveError::RefNotFound)?,
                        );
                        return Ok(());
                    }
                },
            };
            let num_str = format!("{}", raw);
            out.extend_from_slice(num_str.as_bytes());
        }
        _ => out.push(b'%'),
    }
    Ok(())
}

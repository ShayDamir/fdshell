use core::fmt::Write;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn collect_name(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut name = ShortCStr::new();
    name.push(peek.next().ok_or(ResolveError::RefNotFound)?)
        .change_context(ResolveError::NulByte)?;
    while let Some(&nc) = peek.peek() {
        if nc.is_ascii_alphanumeric() || nc == b'_' {
            name.push(nc).change_context(ResolveError::NulByte)?;
            peek.next();
        } else {
            break;
        }
    }
    Ok(name)
}

pub(crate) fn percent_subst(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    match peek.peek().copied() {
        Some(b'%') => {
            out.push_cstr(c"%");
            peek.next();
        }
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = collect_name(peek)?;
            match cache.get(&name_scs) {
                Some(d) => {
                    core::write!(out, "{}", d).change_context(ResolveError::Never)?;
                }
                None => match state.fds.get(&name_scs) {
                    Some(src) => {
                        let owned = src.export().change_context(ResolveError::RefNotFound)?;
                        core::write!(out, "{}", owned).change_context(ResolveError::Never)?;
                        cache.insert(name_scs, owned);
                    }
                    None => {
                        out.push_cstr(c"%");
                        out.push_str(&name_scs);
                        return Ok(());
                    }
                },
            }
        }
        _ => out.push_cstr(c"%"),
    }
    Ok(())
}

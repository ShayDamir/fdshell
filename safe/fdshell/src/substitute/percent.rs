use alloc::string::ToString;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn collect_name(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut name = alloc::vec::Vec::new();
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
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    match peek.peek().copied() {
        Some(b'%') => {
            out.push(b'%').change_context(ResolveError::Never)?;
            peek.next();
        }
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = collect_name(peek)?;
            let num_str = match cache.get(&name_scs) {
                Some(d) => d.to_string(),
                None => match state.fds.get(&name_scs) {
                    Some(src) => {
                        let owned = src.export().change_context(ResolveError::RefNotFound)?;
                        let s = owned.to_string();
                        cache.insert(name_scs, owned);
                        s
                    }
                    None => {
                        out.push(b'%').change_context(ResolveError::Never)?;
                        out.push_str(&name_scs)
                            .change_context(ResolveError::Never)?;
                        return Ok(());
                    }
                },
            };
            out.push_slice(num_str.as_bytes())
                .change_context(ResolveError::NulByte)?;
        }
        _ => out.push(b'%').change_context(ResolveError::NulByte)?,
    }
    Ok(())
}

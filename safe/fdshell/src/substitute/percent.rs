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
    name.push_byte(peek.next().ok_or(ResolveError::RefNotFound)?)
        .change_context(ResolveError::NulByte)?;
    while let Some(&nc) = peek.peek()
        && (nc.is_ascii_alphanumeric() || nc == b'_')
    {
        name.push_byte(nc).change_context(ResolveError::NulByte)?;
        peek.next();
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
            out.push(c"%");
            peek.next();
        }
        // `%?` is the `wait` arm's matched fd; a name in the fd namespace.
        Some(b'?') => wait_fd_subst(peek, cache, state, out)?,
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = collect_name(peek)?;
            if !write_fd(&name_scs, cache, state, out)? {
                out.push(c"%");
                out.push(&name_scs);
            }
        }
        _ => out.push(c"%"),
    }
    Ok(())
}

/// Write `name`'s fd value to `out` (export cache first, then the shell's
/// fd variables, cached on first use); `false` when `name` is not set.
fn write_fd(
    name: &ShortCStr,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<bool, Report<ResolveError>> {
    if let Some(d) = cache.get(name) {
        core::write!(out, "{}", d).change_context(ResolveError::Never)?;
        return Ok(true);
    }
    let Some(src) = state.fds.get(name) else {
        return Ok(false);
    };
    let owned = src.fd.export().change_context(ResolveError::RefNotFound)?;
    core::write!(out, "{}", owned).change_context(ResolveError::Never)?;
    cache.insert(name.clone(), owned);
    Ok(true)
}

/// Substitute the `wait` arm's matched fd bound under the reserved name `?`.
fn wait_fd_subst(
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    let name = ShortCStr::from(c"?");
    if !write_fd(&name, cache, state, out)? {
        out.push(c"%");
        out.push_byte(b'?').change_context(ResolveError::NulByte)?;
    }
    Ok(())
}

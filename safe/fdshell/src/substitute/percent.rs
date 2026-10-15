use core::fmt::Write;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// The name of a `$name` / `%name` arm: a run of at most `limit` name bytes,
/// greedy as bash's (`$vbc` is the parameter `vbc`). `limit` is the trigger
/// byte's quote region, so a name never crosses out of its own region.
pub(crate) fn collect_name(
    peek: &mut super::mask::Counting<'_>,
    limit: usize,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut name = ShortCStr::new();
    for _ in 0..limit {
        let Some(&c) = peek.peek() else { break };
        if !(c.is_ascii_alphanumeric() || c == b'_') {
            break;
        }
        peek.next();
        name.push_byte(c).change_context(ResolveError::NulByte)?;
    }
    Ok(name)
}

/// Substitute the `%` arm's text at `out`: a bound fd name (`%name`, `%?`)
/// writes its fd number, anything else writes a literal `%` and consumes
/// exactly the trigger byte. There is deliberately **no** shell-level `%%`
/// escape — `%%` belongs to the `printf` builtin, which interprets it, so the
/// shell must pass both bytes through (bash parity, task #155, LESSONS). The
/// literal-`%` escape is the POSIX #4.1 pair outside quotes (`\%name`).
/// `limit` caps the name at the trigger byte's quote region, the same cap the
/// `$` arm applies (`"%a"b` is the fd `a` plus the literal `b`).
pub(crate) fn percent_subst(
    peek: &mut super::mask::Counting<'_>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    state: &ShellState,
    out: &mut ShortCStr,
    limit: usize,
) -> Result<(), Report<ResolveError>> {
    match peek.peek().copied() {
        // `%?` is the `wait` arm's matched fd; a name in the fd namespace.
        Some(b'?') => wait_fd_subst(peek, cache, state, out)?,
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = collect_name(peek, limit)?;
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
    peek: &mut super::mask::Counting<'_>,
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

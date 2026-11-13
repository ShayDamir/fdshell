//! The strict-mode arms of redirect opening (README "Strict mode").
//!
//! Strict mode gives redirects the same capability model as the `*at`
//! builtins: every target is opened through the shell's pinned `%CWD` fd and
//! an absolute target (filesystem-root resolution) is banned. Non-strict
//! redirect opening (redirect/open.rs) is byte-for-byte unchanged.

// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

mod openat;

use super::target;
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{RedirectDef, RedirectSource};
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

/// The pinned `%CWD` dirfd clone strict mode opens every target through;
/// `None` for non-strict (the process-CWD `open()` calls, unchanged) and for
/// the defensive case of an unseeded `%CWD` (startup is the only producer).
pub(super) fn strict_base(
    cell: &ForkCell<ShellState>,
    strict: bool,
) -> Result<Option<LocalFd>, Report<OpenRedirectError>> {
    if !strict {
        return Ok(None);
    }
    let state = cell.borrow().change_context(OpenRedirectError::Never)?;
    state
        .cwd_fd()
        .map(|fd| fd.try_clone().change_context(OpenRedirectError::Open))
        .transpose()
}

/// One path redirect target under strict: expand and glob it rooted at the
/// pinned `%CWD` fd `base` (the absolute ban runs on every **expanded**
/// field), open it through `base`, clone it above every redirect target of the
/// command (a `dup2` of another redirect must never clobber the open fd and
/// then have this descriptor's drop close the target).
pub(super) fn target(
    def: &RedirectDef,
    source: &RedirectSource,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    base: &LocalFd,
    min_fd: i32,
    noclobber: bool,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let target_name = target::resolve(source, cache, Some(base), cell)?;
    let name = target_name.export();
    let noclobber_flag = noclobber && def.write_flag();
    let opened = match noclobber_flag {
        true => openat::noclobber_open(def, &name, &target_name, base),
        false => openat::open(&name, &openat::open_how(def), base),
    }?;
    let rehomed = opened
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    drop(opened);
    Ok(rehomed)
}

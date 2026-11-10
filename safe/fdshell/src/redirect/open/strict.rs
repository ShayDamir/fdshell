//! The strict-mode arms of redirect opening (README "Strict mode").
//!
//! Strict mode gives redirects the same capability model as the `*at`
//! builtins: every target is opened through the shell's pinned `%CWD` fd and
//! an absolute target (filesystem-root resolution) is banned. Non-strict
//! redirect opening (redirect/open.rs) is byte-for-byte unchanged.

// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

use super::noclobber::{Decision, decide_from_base};
use super::target;
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::AtFd;
use sys::ExportedCStr;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::SyscallError;
use sys::fcntl::O_CREAT;
use sys::fork_cell::ForkCell;
use sys::openat2::OpenHow;

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
        true => noclobber_open(def, &name, &target_name, base),
        false => open(&name, &open_how(def), base),
    }?;
    let rehomed = opened
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    drop(opened);
    Ok(rehomed)
}

/// The `openat2` form of a redirect open: the direction's flags, and the
/// `O_CREAT` mode `open()` passes (`0o666`, masked by the umask).
fn open_how(def: &RedirectDef) -> OpenHow {
    let flags = def.direction.open_flags();
    let mode = i32::from(flags & O_CREAT != 0) * 0o666;
    OpenHow::new(flags as u64, mode as u64)
}

/// The strict redirect open: `openat2` on the pinned `%CWD` fd, with the
/// direction's flags and the `O_CREAT` mode `open()` passes (`O_CLOEXEC` is
/// forced by `sys`). Non-strict opens from the process CWD (redirect/open/cwd.rs).
fn open(
    name: &ExportedCStr,
    how: &OpenHow,
    base: &LocalFd,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    sys::openat2::openat2(AtFd::from(base), name, how).change_context(OpenRedirectError::Open)
}

/// bash's stat gate through the pinned dirfd: refuse a regular file, open every
/// other existing target with the direction's own flags, and keep the exclusive
/// create for "not there" (redirect/open/noclobber.rs). The refusal names the
/// **expanded** target (bash: `set -C; n=f; echo hi > $n` → `f: cannot
/// overwrite`).
fn noclobber_open(
    def: &RedirectDef,
    name: &ExportedCStr,
    path: &ShortCStr,
    base: &LocalFd,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match decide_from_base(base, name) {
        Decision::Refuse => Err(OpenRedirectError::Noclobber { name: path.clone() }.into()),
        Decision::Plain => open(name, &open_how(def), base),
        // The exclusive create through the pinned dirfd: `EEXIST` is the refusal.
        Decision::Exclusive => {
            let how = OpenHow::new(RedirectDirection::noclobber_flags() as u64, 0o666);
            match open(name, &how, base) {
                Ok(fd) => Ok(fd),
                Err(report) => match report.downcast_ref::<SyscallError>() {
                    Some(e) if e.errno() == sys::errno::EEXIST => {
                        Err(OpenRedirectError::Noclobber { name: path.clone() }.into())
                    }
                    Some(_) | None => Err(report),
                },
            }
        }
    }
}

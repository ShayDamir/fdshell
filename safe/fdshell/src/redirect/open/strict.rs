//! The strict-mode arms of redirect opening (README "Strict mode").
//!
//! Strict mode gives redirects the same capability model as the `*at`
//! builtins: every target is opened through the shell's pinned `%CWD` fd and
//! an absolute target (filesystem-root resolution) is banned. Non-strict
//! redirect opening (redirect/open.rs) is byte-for-byte unchanged.

use super::open_target;
use super::target;
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use error_stack::{Report, ResultExt};
use sys::ExportedCStr;
use sys::LocalFd;
use sys::ShortCStr;
use sys::SyscallError;
use sys::fcntl::O_CREAT;
use sys::fork_cell::ForkCell;
use sys::openat2::OpenHow;

use crate::state::ShellState;

/// One path redirect target under strict: glob it rooted at the pinned `%CWD`
/// fd `base`, open it through `base`, clone it above every redirect target of
/// the command (a `dup2` of another redirect must never clobber the open fd
/// and then have this descriptor's drop close the target).
pub(super) fn target(
    def: &RedirectDef,
    base: &LocalFd,
    min_fd: i32,
    noclobber: bool,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let (path, mask) = match &def.source {
        RedirectSource::Path { path, mask } => (path, mask.as_slice()),
        _ => return Err(Report::new(OpenRedirectError::Never)),
    };
    let target_name = target::resolve(path, mask, Some(base), cell)?;
    let name = target_name.export();
    let noclobber_flag = noclobber && def.write_flag();
    let opened = match noclobber_flag {
        true => noclobber_open(&name, path, base),
        false => open_target(&name, &open_how(def), Some(base)),
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

/// `noclobber` under strict: an exclusive create through the pinned dirfd;
/// `EEXIST` means the target exists.
fn noclobber_open(
    name: &ExportedCStr,
    path: &ShortCStr,
    base: &LocalFd,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let how = OpenHow::new(RedirectDirection::noclobber_flags() as u64, 0o666);
    match open_target(name, &how, Some(base)) {
        Ok(fd) => Ok(fd),
        Err(report) => match report.downcast_ref::<SyscallError>() {
            Some(e) if e.errno() == sys::errno::EEXIST => {
                Err(OpenRedirectError::Noclobber { name: path.clone() }.into())
            }
            Some(_) | None => Err(report),
        },
    }
}

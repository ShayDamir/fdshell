//! The `openat2` plumbing behind a strict-mode redirect open: the `open_how`
//! the direction asks for, the open through the pinned `%CWD` dirfd, and the
//! noclobber decision's own open. The decision gate itself is
//! `redirect/open/noclobber.rs`; the entry points stay in `strict.rs`.

// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

use crate::error::redirect::OpenRedirectError;
use crate::redirect::open::noclobber::{Decision, decide_from_base};
use crate::redirect::{RedirectDef, RedirectDirection};
use error_stack::{Report, ResultExt};
use sys::AtFd;
use sys::ExportedCStr;
use sys::LocalFd;
use sys::ShortCStr;
use sys::SyscallError;
use sys::fcntl::O_CREAT;
use sys::openat2::OpenHow;

/// The `openat2` form of a redirect open: the direction's flags, and the
/// `O_CREAT` mode `open()` passes (`0o666`, masked by the umask).
pub(super) fn open_how(def: &RedirectDef) -> OpenHow {
    let flags = def.direction.open_flags();
    let mode = i32::from(flags & O_CREAT != 0) * 0o666;
    OpenHow::new(flags as u64, mode as u64)
}

/// The strict redirect open: `openat2` on the pinned `%CWD` fd, with the
/// direction's flags and the `O_CREAT` mode `open()` passes (`O_CLOEXEC` is
/// forced by `sys`). Non-strict opens from the process CWD (redirect/open/cwd.rs).
pub(super) fn open(
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
pub(super) fn noclobber_open(
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

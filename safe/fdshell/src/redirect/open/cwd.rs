//! The non-strict redirect open: the process-CWD `open()` calls, byte-for-byte
//! what they were before strict mode existed (redirect/open/strict.rs is the
//! capability route).

// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

use super::noclobber::{Decision, decide_from_path};
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{RedirectDef, RedirectDirection};
use error_stack::{Report, ResultExt};
use sys::ExportedCStr;
use sys::LocalFd;
use sys::ShortCStr;

/// Open the **resolved** target name (the caller's `target::resolve` pass —
/// expand, fold, glob) from the process CWD with the direction's flags —
/// `noclobber` runs bash's stat gate on it (redirect/open/noclobber.rs).
pub(super) fn open(
    def: &RedirectDef,
    path: &ShortCStr,
    noclobber: bool,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let name = path.export();
    if noclobber {
        return noclobber_open(def, &name, path);
    }
    sys::openat2::open(&name, def.direction.open_flags()).change_context(OpenRedirectError::Open)
}

/// bash's stat gate on this route: refuse a regular file, open every other
/// existing target with the direction's own flags, and keep the exclusive
/// create for "not there". The refusal names the **expanded** target, as bash
/// does (`set -C; n=f; echo hi > $n` → `f: cannot overwrite existing file`).
fn noclobber_open(
    def: &RedirectDef,
    name: &ExportedCStr,
    path: &ShortCStr,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match decide_from_path(name) {
        Decision::Refuse => Err(OpenRedirectError::Noclobber { name: path.clone() }.into()),
        Decision::Plain => sys::openat2::open(name, def.direction.open_flags())
            .change_context(OpenRedirectError::Open),
        // Open a fresh file exclusively: `EEXIST` means the target exists.
        Decision::Exclusive => match sys::openat2::open(name, RedirectDirection::noclobber_flags())
        {
            Ok(fd) => Ok(fd),
            Err(e) if e.errno() == sys::errno::EEXIST => {
                Err(OpenRedirectError::Noclobber { name: path.clone() }.into())
            }
            Err(e) => Err(Report::new(OpenRedirectError::Open).attach(e)),
        },
    }
}

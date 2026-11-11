//! The non-strict redirect open: the process-CWD `open()` calls, byte-for-byte
//! what they were before strict mode existed (redirect/open/strict.rs is the
//! capability route).

// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

use super::noclobber::{Decision, decide_from_path};
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{RedirectDef, RedirectDirection};
use error_stack::{Report, ResultExt};
use sys::LocalFd;
use sys::ShortCStr;

/// bash's stat gate on this route (redirect/open/noclobber.rs), exposed so the
/// caller can ask the decision **without** taking the plain arm: task #482 — a
/// magic word's plain open belongs to the resolver, while the refusal and the
/// exclusive create stay the pass's.
pub(super) fn gate(path: &ShortCStr) -> Decision {
    decide_from_path(&path.export())
}

/// Open the **resolved** target name (the caller's `target::resolve` pass —
/// expand, fold, glob) from the process CWD, dispatching on the decision:
/// bash's stat gate's arms when the caller ran `gate`, the plain arm with the
/// direction's flags when it runs no gate (`Decision::Plain` is that route
/// too). The refusal names the **expanded** target, as bash does
/// (`set -C; n=f; echo hi > $n` → `f: cannot overwrite existing file`).
pub(super) fn open(
    def: &RedirectDef,
    path: &ShortCStr,
    decision: Decision,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let name = path.export();
    match decision {
        Decision::Refuse => Err(OpenRedirectError::Noclobber { name: path.clone() }.into()),
        Decision::Plain => sys::openat2::open(&name, def.direction.open_flags())
            .change_context(OpenRedirectError::Open),
        // Open a fresh file exclusively: `EEXIST` means the target exists.
        Decision::Exclusive => {
            match sys::openat2::open(&name, RedirectDirection::noclobber_flags()) {
                Ok(fd) => Ok(fd),
                Err(e) if e.errno() == sys::errno::EEXIST => {
                    Err(OpenRedirectError::Noclobber { name: path.clone() }.into())
                }
                Err(e) => Err(Report::new(OpenRedirectError::Open).attach(e)),
            }
        }
    }
}

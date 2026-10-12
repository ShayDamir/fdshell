//! The non-strict redirect open: the process-CWD `open()` calls, byte-for-byte
//! what they were before strict mode existed (redirect/open/strict.rs is the
//! capability route).

use crate::error::redirect::OpenRedirectError;
use crate::redirect::{RedirectDef, RedirectDirection};
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ExportedCStr;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::target;

/// Open a redirect target from the process CWD: resolve the word (fold + glob),
/// then `open()` it with the direction's flags — `noclobber` turns that into an
/// exclusive create.
pub(super) fn open(
    def: &RedirectDef,
    path: &ShortCStr,
    mask: &[bool],
    noclobber: bool,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let target = target::resolve(path, mask, None, cell)?;
    let name = target.export();
    if noclobber {
        return noclobber_open(&name, path);
    }
    sys::openat2::open(&name, def.direction.open_flags()).change_context(OpenRedirectError::Open)
}

/// Open a fresh file exclusively: `EEXIST` means the target exists (noclobber).
fn noclobber_open(
    name: &ExportedCStr,
    path: &ShortCStr,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match sys::openat2::open(name, RedirectDirection::noclobber_flags()) {
        Ok(fd) => Ok(fd),
        Err(e) if e.errno() == sys::errno::EEXIST => {
            Err(OpenRedirectError::Noclobber { name: path.clone() }.into())
        }
        Err(e) => Err(Report::new(OpenRedirectError::Open).attach(e)),
    }
}

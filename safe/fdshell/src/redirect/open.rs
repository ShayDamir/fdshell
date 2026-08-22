use super::RedirectDef;
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ExportedCStr;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fcntl::{O_CREAT, O_EXCL, O_WRONLY};
use sys::fork_cell::ForkCell;

pub fn open_redirect_files(
    redirects: &[RedirectDef],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<LocalFd>, Report<OpenRedirectError>> {
    let state = cell.borrow().change_context(OpenRedirectError::Never)?;
    let noclobber = state.options & crate::options::NOCLOBBER != 0;
    let mut fds = Vec::new();
    for r in redirects {
        if let super::RedirectSource::Path(path) = &r.source {
            let name = path.export();
            if noclobber && matches!(r.direction, super::RedirectDirection::Write) {
                fds.push(open_noclobber(&name, path)?);
                continue;
            }
            fds.push(
                sys::openat2::open(&name, r.direction.open_flags())
                    .change_context(OpenRedirectError::Open)?,
            );
        }
    }
    Ok(fds)
}

/// Open a fresh file exclusively: `EEXIST` means the target exists (noclobber).
fn open_noclobber(
    name: &ExportedCStr,
    path: &ShortCStr,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match sys::openat2::open(name, O_WRONLY + O_CREAT + O_EXCL) {
        Ok(fd) => Ok(fd),
        Err(e) if e.errno() == sys::errno::EEXIST => {
            Err(OpenRedirectError::Noclobber { name: path.clone() }.into())
        }
        Err(e) => Err(Report::new(OpenRedirectError::Open).attach(e)),
    }
}

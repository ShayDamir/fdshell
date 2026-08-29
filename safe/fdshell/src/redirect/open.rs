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
    let min_fd = RedirectDef::max_target(redirects)
        .checked_add(1)
        .ok_or(OpenRedirectError::FdNumberOutOfRange)?;
    let mut fds = Vec::new();
    for r in redirects {
        if let super::RedirectSource::Path(path) = &r.source {
            let name = path.export();
            let opened = if noclobber && matches!(r.direction, super::RedirectDirection::Write) {
                open_noclobber(&name, path)?
            } else {
                sys::openat2::open(&name, r.direction.open_flags())
                    .change_context(OpenRedirectError::Open)?
            };
            // Re-home the freshly opened fd above every redirect target of this
            // command: a `dup2` of another redirect must never clobber the open
            // fd and then have this descriptor's drop close the target.
            let rehomed = opened
                .try_clone_above(min_fd)
                .change_context(OpenRedirectError::Open)?;
            drop(opened);
            fds.push(rehomed);
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

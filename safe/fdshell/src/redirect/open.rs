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
    // Read `noclobber` in a block-scoped borrow: `matches` re-borrows the cell
    // per redirect target (RefCell, LESSONS.md).
    let noclobber = {
        let state = cell.borrow().change_context(OpenRedirectError::Never)?;
        state.options & crate::options::NOCLOBBER != 0
    };
    let min_fd = RedirectDef::max_target(redirects)
        .checked_add(1)
        .ok_or(OpenRedirectError::FdNumberOutOfRange)?;
    let mut fds = Vec::new();
    for r in redirects {
        if let super::RedirectSource::Path { path, mask } = &r.source {
            let target = redirect_target(path, mask, cell)?;
            let name = target.export();
            let opened = if noclobber && matches!(r.direction, super::RedirectDirection::Write) {
                open_noclobber(&name, &target)?
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

/// Glob a redirect target: `> 1` match is ambiguous, `0` matches open the
/// literal word (bash passes unmatched redirect patterns through; `failglob`
/// does not apply to redirects), `1` match opens that match.
fn redirect_target(
    path: &ShortCStr,
    mask: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<OpenRedirectError>> {
    let matches = crate::glob::matches(path, mask, cell).change_context(OpenRedirectError::Glob)?;
    match matches.len() {
        0 => Ok(path.clone()),
        1 => matches
            .first()
            .cloned()
            .ok_or(OpenRedirectError::Never.into()),
        _ => Err(Report::new(OpenRedirectError::AmbiguousRedirect {
            name: path.clone(),
        })),
    }
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

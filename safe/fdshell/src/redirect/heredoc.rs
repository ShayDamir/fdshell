use crate::error::redirect::OpenRedirectError;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fcntl::SEEK_SET;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

/// Back the here-doc body with a seeked-to-zero memfd as the new stdin.
/// Unlike a here-string, no newline is appended: the body bytes are exactly
/// the lines between the command line and the delimiter line.
pub fn here_doc(
    body: &ShortCStr,
    expand: bool,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let data = if expand {
        crate::substitute::substitute_arg(body, &[], cache, cell)
            .change_context(OpenRedirectError::HereDocExpand)?
            .0
    } else {
        body.clone()
    };
    let fd = sys::memfd::memfd_create().change_context(OpenRedirectError::HereDocCreate)?;
    let bytes = data.as_bytes().change_context(OpenRedirectError::Never)?;
    sys::rw::write_all(&fd, bytes).change_context(OpenRedirectError::HereDocCreate)?;
    sys::rw::lseek(&fd, 0, SEEK_SET).change_context(OpenRedirectError::HereDocCreate)?;
    Ok(fd)
}

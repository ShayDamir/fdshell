use crate::error::redirect::OpenRedirectError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fcntl::SEEK_SET;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

/// Expand the word and back it with a seeked-to-zero memfd as the new stdin.
pub fn here_string(
    word: &ShortCStr,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let (data, _) = crate::substitute::substitute_arg(word, &[], cache, cell)
        .change_context(OpenRedirectError::HereStringExpand)?;
    let fd = sys::memfd::memfd_create().change_context(OpenRedirectError::HereStringCreate)?;
    let mut payload = Vec::with_capacity(data.len() + 1);
    payload.extend_from_slice(data.as_bytes().change_context(OpenRedirectError::Never)?);
    payload.push(b'\n');
    sys::rw::write_all(&fd, &payload).change_context(OpenRedirectError::HereStringCreate)?;
    sys::rw::lseek(&fd, 0, SEEK_SET).change_context(OpenRedirectError::HereStringCreate)?;
    Ok(fd)
}

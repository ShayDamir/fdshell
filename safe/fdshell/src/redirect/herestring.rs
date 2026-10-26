use crate::error::redirect::OpenRedirectError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::mask::MaskBit;
use crate::state::ShellState;

/// Expand the word **with its own quote mask** and back it with a memfd
/// re-opened `O_RDONLY` (a fresh open file description, offset 0) as the new
/// stdin, so the descriptor this redirect opens is read-only and a `>&N` write
/// to it is refused `EBADF`, as bash's here-string source is. The appended
/// newline is payload, not a word byte, so it takes no mask bit.
pub fn here_string(
    word: &ShortCStr,
    mask: &[MaskBit],
    spans: &[usize],
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let (data, _) = crate::substitute::substitute_arg(word, mask, spans, cache, cell)
        .change_context(OpenRedirectError::HereStringExpand)?;
    let fd = sys::memfd::memfd_create().change_context(OpenRedirectError::HereStringCreate)?;
    let mut payload = Vec::with_capacity(data.len() + 1);
    payload.extend_from_slice(data.as_bytes().change_context(OpenRedirectError::Never)?);
    payload.push(b'\n');
    fd.write_all(&payload)
        .change_context(OpenRedirectError::HereStringCreate)?;
    // The re-open carries the export mode: a fresh, read-only description at
    // offset 0, so the memfd needs no `lseek` and the writable fd dies here
    // (task #325).
    fd.reopen_read_only()
        .change_context(OpenRedirectError::HereStringCreate)
}

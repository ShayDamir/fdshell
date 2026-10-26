use crate::error::redirect::OpenRedirectError;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::mask::MaskBit;
use crate::state::ShellState;

/// The body's quote mask: an expandable here-doc body **is** a quoted context
/// (POSIX 2.2: "the here-document is equivalent to double-quoting"), so
/// `\<char>` keeps its backslash, `\$`/`\\`/`` \` `` drop it, `$( … )` and
/// backticks still expand, and a leading `~` stays literal (the mask gate of
/// `substitute/tilde.rs`, task #269). `\<newline>` is never seen here: the
/// parse layer removed those continuations (`parse/heredoc/strip.rs`).
fn quoted_context(len: usize) -> Vec<MaskBit> {
    vec![MaskBit::Quoted; len]
}

/// Back the here-doc body with a memfd re-opened `O_RDONLY` (a fresh open file
/// description, offset 0) as the new stdin, so the descriptor this redirect
/// opens is read-only and a `>&N` write to it is refused `EBADF`, as bash's
/// here-document source is.
/// Unlike a here-string, no newline is appended: the body bytes are exactly
/// the lines between the command line and the delimiter line.
pub fn here_doc(
    body: &ShortCStr,
    expand: bool,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let data = if expand {
        // The body is raw data, not a shell word: its quote characters are not
        // shell quotes, so it has no zero-byte quote spans (task #321).
        crate::substitute::substitute_arg(body, &quoted_context(body.len()), &[], cache, cell)
            .change_context(OpenRedirectError::HereDocExpand)?
            .0
    } else {
        body.clone()
    };
    let fd = sys::memfd::memfd_create().change_context(OpenRedirectError::HereDocCreate)?;
    let bytes = data.as_bytes().change_context(OpenRedirectError::Never)?;
    fd.write_all(bytes)
        .change_context(OpenRedirectError::HereDocCreate)?;
    // The re-open carries the export mode: a fresh, read-only description at
    // offset 0, so the memfd needs no `lseek` and the writable fd dies here
    // (task #325).
    fd.reopen_read_only()
        .change_context(OpenRedirectError::HereDocCreate)
}

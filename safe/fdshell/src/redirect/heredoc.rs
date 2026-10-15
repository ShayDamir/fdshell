use crate::error::redirect::OpenRedirectError;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fcntl::SEEK_SET;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

/// The body's quote mask: an expandable here-doc body **is** a quoted context
/// (POSIX 2.2: "the here-document is equivalent to double-quoting"), so
/// `\<char>` keeps its backslash, `\$`/`\\`/`` \` `` drop it, `$( … )` and
/// backticks still expand, and a leading `~` stays literal (the mask gate of
/// `substitute/tilde.rs`, task #269). `\<newline>` is never seen here: the
/// parse layer removed those continuations (`parse/heredoc/strip.rs`).
fn quoted_context(len: usize) -> Vec<bool> {
    vec![true; len]
}

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
        crate::substitute::substitute_arg(body, &quoted_context(body.len()), cache, cell)
            .change_context(OpenRedirectError::HereDocExpand)?
            .0
    } else {
        body.clone()
    };
    let fd = sys::memfd::memfd_create().change_context(OpenRedirectError::HereDocCreate)?;
    let bytes = data.as_bytes().change_context(OpenRedirectError::Never)?;
    fd.write_all(bytes)
        .change_context(OpenRedirectError::HereDocCreate)?;
    fd.lseek(0, SEEK_SET)
        .change_context(OpenRedirectError::HereDocCreate)?;
    Ok(fd)
}

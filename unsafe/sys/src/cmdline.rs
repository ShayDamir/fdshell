//! `/proc/self/cmdline` reader.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};

use crate::shortcstr::ShortCStr;

/// Error type for `read_cmdline`.
#[derive(Debug, displaydoc::Display)]
pub enum ReadCmdlineError {
    /// failed to open /proc/self/cmdline
    OpenFailed,
    /// argument contains a NUL byte
    InvalidArg,
    /// command line is empty (missing argv[0])
    EmptyCmdline,
    /// impossible state (indexing invariant violation)
    Never,
}

impl core::error::Error for ReadCmdlineError {}

/// Read `/proc/self/cmdline` and split on NUL bytes.
///
/// Returns each argument as a `ShortCStr`. The command line must contain at least `argv[0]`.
pub fn read_cmdline() -> Result<Vec<ShortCStr>, Report<ReadCmdlineError>> {
    use crate::fcntl::O_RDONLY;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let fd = crate::openat2::open(c"/proc/self/cmdline", O_RDONLY)
        .change_context(ReadCmdlineError::OpenFailed)?;
    loop {
        let n = crate::rw::read(&fd, &mut chunk).change_context(ReadCmdlineError::OpenFailed)?;
        if n == 0 {
            break;
        }
        let slice = chunk.get(..n).ok_or(ReadCmdlineError::Never)?;
        buf.extend_from_slice(slice);
    }
    ensure!(!buf.is_empty(), ReadCmdlineError::EmptyCmdline);
    let mut parts: Vec<ShortCStr> = buf
        .split(|&b| b == b'\0')
        .map(|f| ShortCStr::from_vec(f.to_vec()))
        .collect::<Result<Vec<_>, _>>()
        .change_context(ReadCmdlineError::InvalidArg)?;
    if parts.last().is_some_and(|p| p.is_empty()) {
        let _ = parts.pop();
    }
    Ok(parts)
}

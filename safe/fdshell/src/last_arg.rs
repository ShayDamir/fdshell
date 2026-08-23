//! `$_` reporting: a foreground child reports its last expanded word (or the
//! command name when it has no arguments) to the parent over the capture
//! socket, as a memfd tagged `$_`. The parent stores it in the `_` variable.

use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt, ensure};

use crate::error::child_process::ChildProcessError;
use crate::error::launch::LaunchError;
use sys::Pid;
use sys::ShortCStr;

const TAG: &[u8] = b"$_";

/// Send `word` to the parent as the `$_` message.
pub(crate) fn send(sock: &sys::LocalFd, word: &ShortCStr) -> Result<(), Report<ChildProcessError>> {
    let mem = sys::memfd::memfd_create().change_context(ChildProcessError::LastArgSend)?;
    let bytes = word
        .as_bytes()
        .change_context(ChildProcessError::LastArgSend)?;
    sys::rw::write_all(&mem, bytes).change_context(ChildProcessError::LastArgSend)?;
    sys::shellfd::send_fd(sock, &mem, c"$_").change_context(ChildProcessError::LastArgSend)?;
    Ok(())
}

/// Receive the `$_` message from `child_pid`. `Ok(None)` when the child
/// exited without sending one (substitution failed).
pub(crate) fn recv(
    sock: &sys::LocalFd,
    child_pid: Pid,
) -> Result<Option<ShortCStr>, Report<LaunchError>> {
    let mut tag_buf = [0u8; sys::shellfd::TAG_MAX];
    let (fd, tag) = match sys::shellfd::recv_fd(sock, &mut tag_buf, child_pid) {
        Ok(v) => v,
        Err(e) => {
            if matches!(e.current_context(), sys::RecvFdError::Closed) {
                return Ok(None);
            }
            return Err(e).change_context(LaunchError::LastArg);
        }
    };
    ensure!(tag.to_bytes() == TAG, LaunchError::Never);
    // The memfd's offset is shared with the sender, which left it at EOF.
    sys::rw::lseek(&fd, 0, sys::fcntl::SEEK_SET).change_context(LaunchError::LastArg)?;
    let word = read_all(&fd)?;
    ShortCStr::from_vec(word)
        .map(Some)
        .change_context(LaunchError::LastArg)
}

/// Reserved tag: capture matching must never treat it as a user capture.
pub(crate) fn is_tag(tag: &CStr) -> bool {
    tag.to_bytes() == TAG
}

fn read_all(fd: &sys::LocalFd) -> Result<Vec<u8>, Report<LaunchError>> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = sys::rw::read(fd, &mut buf).change_context(LaunchError::LastArg)?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(buf.get(..n).ok_or(LaunchError::Never)?);
    }
    Ok(data)
}

#[cfg(test)]
mod tests;

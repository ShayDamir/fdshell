//! `memfd` builtin: create a sealed in-memory file and export it by fd.
//!
//! `memfd [--name NAME] [--size BYTES] [--seal FLAG]…` creates an anonymous
//! memfd with `MFD_CLOEXEC | MFD_ALLOW_SEALING` set, optionally grows it with
//! `ftruncate` to `--size`, optionally seals it with one or more `--seal`
//! `F_SEAL_*` masks, and sends the handle to the parent shell via the capture
//! socket tagged `memfd`. The parent binds it to a `%>memfd` or `%>%var` fd
//! variable, giving a temp-file-less, path-less backing store.

pub mod parse;

use error_stack::{Report, ResultExt};
use sys::LocalFd;

use crate::error::BuiltinError;

/// Create the memfd, size and seal it, and export it to the parent shell.
pub fn memfd_exec(
    cfg: &parse::MemfdConfig<'_>,
    sock: &LocalFd,
) -> Result<(), Report<BuiltinError>> {
    let fd = sys::memfd::memfd_create_with_name_and_flags(
        cfg.name,
        sys::memfd::MFD_CLOEXEC | sys::memfd::MFD_ALLOW_SEALING,
    )
    .change_context(BuiltinError::Syscall)?;
    if let Some(size) = cfg.size {
        // Grow before sealing: a grown file can still be sealed, but a sealed
        // file cannot grow, so the order matters.
        let len = i64::try_from(size).change_context(BuiltinError::InvalidArgument("size"))?;
        sys::fileops::ftruncate(&fd, len).change_context(BuiltinError::Syscall)?;
    }
    if cfg.seals != 0 {
        sys::memfd::memfd_set_seal(&fd, cfg.seals).change_context(BuiltinError::Syscall)?;
    }
    sys::shellfd::send_fd(sock, &fd, c"memfd").change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}

#[cfg(test)]
mod tests;

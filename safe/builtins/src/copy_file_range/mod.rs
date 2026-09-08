//! `copy_file_range` builtin — zero-copy copy of bytes between two `%var` fds.
//!
//! `copy_file_range %in %out [COUNT]` copies `COUNT` bytes (or all of `%in`
//! when `COUNT` is omitted) from `%in` to `%out` with `copy_file_range(2)`: the
//! kernel copies without routing bytes through userspace, and no path is
//! re-opened. When the fd pair cannot use the kernel copy (cross-filesystem, a
//! pipe, …) the syscall fails and that errno is returned. Prints the number of
//! bytes copied.

pub mod parse;

use error_stack::{Report, ResultExt, bail};
use sys::LocalFd;

use crate::error::BuiltinError;

pub fn copy_file_range_exec(
    cfg: &parse::CopyFileRangeConfig,
    in_fd: &LocalFd,
    out_fd: &LocalFd,
) -> Result<usize, Report<BuiltinError>> {
    // With a `COUNT` the caller wants exactly that many bytes; without one we
    // copy to EOF, so a short read is expected, not an error.
    let require_exact = cfg.count.is_some();
    let total = match cfg.count {
        Some(n) => n,
        None => remaining(in_fd)?,
    };
    copy_bytes(in_fd, out_fd, total, require_exact)
}

/// Bytes available in `fd` from its current position, via `lseek(SEEK_END)`.
///
/// Requires a seekable fd; a non-seekable input (pipe, socket, …) needs an
/// explicit `COUNT` instead, since there is no size to fall back to.
fn remaining(fd: &LocalFd) -> Result<u64, Report<BuiltinError>> {
    let start =
        sys::rw::lseek(fd, 0, sys::fcntl::SEEK_CUR).change_context(BuiltinError::Syscall)?;
    let end = sys::rw::lseek(fd, 0, sys::fcntl::SEEK_END).change_context(BuiltinError::Syscall)?;
    sys::rw::lseek(fd, start, sys::fcntl::SEEK_SET).change_context(BuiltinError::Syscall)?;
    Ok((end - start) as u64)
}

/// Copy `total` bytes, returning the number copied. `copy_file_range` performs
/// the whole copy in a single syscall, so no loop over the bytes is needed.
fn copy_bytes(
    in_fd: &LocalFd,
    out_fd: &LocalFd,
    total: u64,
    require_exact: bool,
) -> Result<usize, Report<BuiltinError>> {
    if total == 0 {
        return Ok(0);
    }
    let copied = sys::fileops::copy_file_range(in_fd, out_fd, total as usize)
        .change_context(BuiltinError::Syscall)?;
    if require_exact && copied < total as usize {
        bail!(BuiltinError::InvalidArgument("count"));
    }
    Ok(copied)
}

#[cfg(test)]
mod tests;

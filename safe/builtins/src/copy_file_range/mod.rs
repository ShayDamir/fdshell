//! `copy_file_range` builtin — zero-copy copy of bytes between two `%var` fds.
//!
//! `copy_file_range %in %out [COUNT]` copies `COUNT` bytes (or all of `%in`
//! when `COUNT` is omitted) from `%in` to `%out` with `copy_file_range(2)`: the
//! kernel copies without routing bytes through userspace, and no path is
//! re-opened. When the fd pair cannot use the kernel copy (cross-filesystem, a
//! pipe, …) the syscall fails and that errno is returned. Prints the number of
//! bytes copied.
//!
//! With an explicit `COUNT` the caller wants exactly that many bytes: a source
//! with fewer available bytes fails with a `count` error, leaving `%out`
//! unchanged — its size is truncated back to the pre-copy size and its offset
//! restored. A destination the kernel copy cannot write to (a pipe,
//! `/dev/null`, …) fails with that syscall's errno instead.

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
/// With `require_exact`, a short copy fails with the destination restored.
fn copy_bytes(
    in_fd: &LocalFd,
    out_fd: &LocalFd,
    total: u64,
    require_exact: bool,
) -> Result<usize, Report<BuiltinError>> {
    if total == 0 {
        return Ok(0);
    }
    if require_exact {
        return copy_exact(in_fd, out_fd, total);
    }
    sys::fileops::copy_file_range(in_fd, out_fd, total as usize)
        .change_context(BuiltinError::Syscall)
}

/// Copy exactly `total` bytes: when the source has fewer, restore the
/// destination's size and offset and fail with `count`.
fn copy_exact(
    in_fd: &LocalFd,
    out_fd: &LocalFd,
    total: u64,
) -> Result<usize, Report<BuiltinError>> {
    let offset =
        sys::rw::lseek(out_fd, 0, sys::fcntl::SEEK_CUR).change_context(BuiltinError::Syscall)?;
    let size =
        sys::rw::lseek(out_fd, 0, sys::fcntl::SEEK_END).change_context(BuiltinError::Syscall)?;
    let copied = sys::fileops::copy_file_range(in_fd, out_fd, total as usize)
        .change_context(BuiltinError::Syscall)?;
    if copied < total as usize {
        restore(out_fd, offset, size)?;
        bail!(BuiltinError::InvalidArgument("count"));
    }
    Ok(copied)
}

/// Undo a short exact copy: truncate `out` back to `size` and restore its
/// offset to `offset`. Only reached after a successful `copy_file_range`, so
/// the destination is a seekable regular file and the truncate cannot fail
/// with a no-size errno.
fn restore(out: &LocalFd, offset: i64, size: i64) -> Result<(), Report<BuiltinError>> {
    sys::fileops::ftruncate(out, size).change_context(BuiltinError::Syscall)?;
    sys::rw::lseek(out, offset, sys::fcntl::SEEK_SET).change_context(BuiltinError::Syscall)?;
    Ok(())
}

#[cfg(test)]
mod tests;

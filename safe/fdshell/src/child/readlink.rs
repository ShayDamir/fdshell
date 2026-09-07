//! `readlink PATH [--dir %d]` or `readlink %fd` — a symlink's target.
//!
//! Path form: `readlinkat(2)` on the path resolved against the directory fd
//! in `%dir` (or the CWD); the target is printed as-is, never re-resolved
//! past the link. Fd form: read the symlink behind the open handle in `%fd`
//! via an empty path, so the result reflects the exact link the handle
//! points at. Prints the target and a newline.

mod parse;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{AtFd, ShortCStr};

/// A symlink target fits in 4 KiB: the kernel caps targets below 4096 bytes.
const BUF: usize = 4096;

pub(super) fn handle_readlink(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let (dirfd, path) = match parse::readlink_parse(refs, args)? {
        parse::Target::Path { path, dir } => (dirfd(dir.as_ref(), state)?, path),
        parse::Target::Fd { var } => (resolve(&var, state)?.at(), c""),
    };
    let mut buf = [0u8; BUF];
    let n = sys::statx::readlinkat(dirfd, path, &mut buf).change_context(BuiltinError::Syscall)?;
    let mut line = ShortCStr::from_vec(buf.get(..n).ok_or(BuiltinError::Never)?.to_vec())
        .change_context(BuiltinError::Never)?;
    line.push_byte(b'\n').change_context(BuiltinError::Never)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

fn dirfd<'a>(
    dir: Option<&ShortCStr>,
    state: &'a ShellState,
) -> Result<AtFd<'a>, Report<BuiltinError>> {
    match dir {
        None => Ok(AtFd::cwd()),
        Some(name) => {
            let found = state.fds.get(name).ok_or(BuiltinError::FdVarNotFound)?;
            Ok(found.fd.at())
        }
    }
}

fn resolve<'a>(
    var: &ShortCStr,
    state: &'a ShellState,
) -> Result<&'a sys::LocalFd, Report<BuiltinError>> {
    let found = state.fds.get(var).ok_or(BuiltinError::FdVarNotFound)?;
    Ok(&found.fd)
}

#[cfg(test)]
mod tests;

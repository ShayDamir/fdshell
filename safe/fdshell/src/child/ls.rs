//! `ls %dirfd` or `ls PATH [--dir %d]` — list a directory's entry names.
//!
//! Lists the names in the directory behind an open `%dirfd` fd-variable, or in
//! the directory at `PATH` (opened read-only, resolved against `--dir %d` or
//! the CWD). Each name is printed on its own line, read straight from
//! `getdents64(2)` so the walk never re-resolves a path.

mod parse;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::openat2::OpenHow;
use sys::{AtFd, LocalFd, OUT, ShortCStr};

pub(super) fn handle_ls(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    match parse::ls_parse(refs, args)? {
        parse::Target::Fd { var } => list(resolve(&var, state)?),
        parse::Target::Path { path, dir } => {
            let dirfd = dirfd(dir.as_ref(), state)?;
            let how = OpenHow::new(sys::fcntl::O_DIRECTORY as u64, 0);
            let fd =
                sys::openat2::openat2(dirfd, path, &how).change_context(BuiltinError::Syscall)?;
            list(&fd)
        }
    }
}

/// Walk the directory, printing one entry name per line.
fn list(fd: &LocalFd) -> Result<i32, Report<BuiltinError>> {
    let mut buf = [0u8; 4096];
    loop {
        let n = sys::getdents64::getdents(fd.as_raw(), &mut buf)
            .change_context(BuiltinError::Syscall)?;
        if n == 0 {
            break;
        }
        for entry in sys::getdents64::Iter::new(&buf, n) {
            OUT.write_all(entry.name).change_context(BuiltinError::Io)?;
            OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
        }
    }
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
) -> Result<&'a LocalFd, Report<BuiltinError>> {
    let found = state.fds.get(var).ok_or(BuiltinError::FdVarNotFound)?;
    Ok(&found.fd)
}

#[cfg(test)]
mod tests;

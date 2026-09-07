//! `statx PATH [--dir %d] [--nofollow]` or `statx %fd` — file metadata.
//!
//! Path form: `statx(2)` on the path resolved against the directory fd in
//! `%dir` (or the CWD). `--nofollow` stats a symlink itself. Fd form: re-stat
//! the open handle behind `%fd` via `AT_EMPTY_PATH`, so the result reflects
//! the exact file the handle points at, never a path. Prints one line:
//! `kind=.. size=.. mode=.. ino=.. dev=MAJ:MIN mtime=..`.

mod emit;
mod parse;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{AtFd, ShortCStr};

pub(super) fn handle_statx(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let target = parse::statx_parse(refs, args)?;
    let st = match target {
        parse::Target::Path {
            path,
            dir,
            nofollow,
        } => {
            let dirfd = dirfd(dir.as_ref(), state)?;
            sys::statx::statx(dirfd, path, flags(nofollow, false))
        }
        parse::Target::Fd { var, nofollow } => {
            let fd = resolve(&var, state)?;
            sys::statx::statx(fd.at(), c"", flags(nofollow, true))
        }
    }
    .change_context(BuiltinError::Syscall)?;
    let line = emit::line(&st).change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

fn flags(nofollow: bool, empty_path: bool) -> i32 {
    let mut f = 0;
    if nofollow {
        f += sys::fcntl::AT_SYMLINK_NOFOLLOW;
    }
    if empty_path {
        f += sys::fcntl::AT_EMPTY_PATH;
    }
    f
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

//! `ls %dirfd` or `ls PATH [--dir %d]` — list a directory's entry names.
//!
//! Lists the names in the directory behind an open `%dirfd` fd-variable, or in
//! the directory at `PATH` (opened read-only, resolved against `--dir %d` or
//! the CWD). Each name is printed on its own line, read straight from
//! `getdents64(2)` so the walk never re-resolves a path.

// mutants: -C --lib -C --test -C ls -- -E 'test(/^child::ls::/) | binary(ls)'

mod parse;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::openat2::OpenHow;
use sys::{AtFd, LocalFd, OUT, ShortCStr};

use super::Ctx;

pub(super) fn handle_ls(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    match parse::ls_parse(ctx.refs, ctx.args)? {
        parse::Target::Fd { var } => {
            let fd = ctx.state.fd_var(&var).ok_or(BuiltinError::FdVarNotFound)?;
            list(&fd.fd)
        }
        parse::Target::Path { path, dir } => {
            let dirfd = dirfd(dir.as_ref(), ctx.state)?;
            let how = OpenHow::new(sys::fcntl::O_DIRECTORY as u64, 0);
            let fd =
                sys::openat2::openat2(dirfd, path, &how).change_context(BuiltinError::Syscall)?;
            list(&fd)
        }
    }
}

/// Walk the directory, printing one entry name per line.
fn list(fd: &LocalFd) -> Result<i32, Report<BuiltinError>> {
    while let Some(batch) = fd.dir_batch().change_context(BuiltinError::Syscall)? {
        for entry in batch.iter() {
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

#[cfg(test)]
mod tests;

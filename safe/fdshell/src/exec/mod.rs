mod environ;
mod search;

pub use search::resolve_path;

use alloc::vec::Vec;
use core::ffi::CStr;
use hashbrown::HashMap;

use error_stack::{Report, ResultExt};
use sys::{AtFd, ExportedCStr, ImportedStr, LocalFd, ShortCStr};

use crate::envfilter::EnvFilter;
use crate::error::child_process::ChildProcessError;

use environ::get_environ;

fn prepare_envp(
    environ: &[(ShortCStr, ShortCStr)],
    exports: &HashMap<ShortCStr, ImportedStr>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<Vec<ExportedCStr>, Report<ChildProcessError>> {
    let exec_sock = shell_sock
        .map(|s| s.export())
        .transpose()
        .change_context(ChildProcessError::ExportFailed)?;
    Ok(get_environ(
        sys::env::getpid(),
        environ,
        exports,
        env_filter,
        exec_sock.as_ref(),
    ))
}

pub fn exec_fd(
    fd: &LocalFd,
    argv: &[&CStr],
    environ: &[(ShortCStr, ShortCStr)],
    exports: &HashMap<ShortCStr, ImportedStr>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let envp = prepare_envp(environ, exports, env_filter, shell_sock)?;
    builtins::execfd::execfd_exec(fd, argv, &envp).change_context(ChildProcessError::ExecFailed)
}

pub fn exec_at(
    dirfd: AtFd<'_>,
    pathname: &CStr,
    argv: &[&CStr],
    environ: &[(ShortCStr, ShortCStr)],
    exports: &HashMap<ShortCStr, ImportedStr>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let envp = prepare_envp(environ, exports, env_filter, shell_sock)?;
    builtins::execat::execat_exec(dirfd, pathname, argv, &envp)
        .change_context(ChildProcessError::ExecFailed)
}

#[cfg(test)]
mod tests;

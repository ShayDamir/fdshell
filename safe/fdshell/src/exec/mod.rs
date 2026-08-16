mod environ;
mod search;

pub use search::resolve_path;

use alloc::vec::Vec;
use core::ffi::CStr;
use hashbrown::HashMap;

use error_stack::{Report, ResultExt};
use sys::execveat::AT_EMPTY_PATH;
use sys::{AtFd, ExportedCStr, LocalFd, ShortCStr};

use crate::envfilter::EnvFilter;
use crate::error::child_process::ChildProcessError;

use environ::get_environ;

fn prepare_envp(
    environ: &[(ShortCStr, ShortCStr)],
    exports: &HashMap<ShortCStr, ShortCStr>,
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
    exports: &HashMap<ShortCStr, ShortCStr>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let envp = prepare_envp(environ, exports, env_filter, shell_sock)?;
    let script_fd = fd
        .export()
        .change_context(ChildProcessError::ExportFailed)?;
    sys::execveat::execveat(script_fd.at(), c"", argv, &envp, AT_EMPTY_PATH)
        .change_context(ChildProcessError::ExecFailed)?;
    Ok(())
}

pub fn exec_at(
    dirfd: AtFd<'_>,
    pathname: &CStr,
    argv: &[&CStr],
    environ: &[(ShortCStr, ShortCStr)],
    exports: &HashMap<ShortCStr, ShortCStr>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let envp = prepare_envp(environ, exports, env_filter, shell_sock)?;
    sys::execveat::execveat(dirfd, pathname, argv, &envp, 0)
        .change_context(ChildProcessError::ExecFailed)?;
    Ok(())
}

#[cfg(test)]
mod tests;

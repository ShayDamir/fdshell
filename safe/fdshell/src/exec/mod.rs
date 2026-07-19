mod environ;

use core::ffi::CStr;
use hashbrown::HashMap;

use error_stack::{Report, ResultExt, bail};
use sys::execveat::AT_EMPTY_PATH;
use sys::fcntl::O_PATH;
use sys::{AtFd, LocalFd, ShortCStr};

use crate::envfilter::EnvFilter;
use crate::error::child_process::ChildProcessError;

use environ::get_environ;

pub fn exec_fd(
    fd: &LocalFd,
    argv: &[&CStr],
    exports: &HashMap<ShortCStr, ShortCStr>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let pid = sys::env::getpid();
    let exec_sock = shell_sock
        .map(|s| s.export())
        .transpose()
        .change_context(ChildProcessError::ExportFailed)?;
    let envp = get_environ(pid, exports, env_filter, exec_sock.as_ref());
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
    exports: &HashMap<ShortCStr, ShortCStr>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let pid = sys::env::getpid();
    let exec_sock = shell_sock
        .map(|s| s.export())
        .transpose()
        .change_context(ChildProcessError::ExportFailed)?;
    let envp = get_environ(pid, exports, env_filter, exec_sock.as_ref());
    sys::execveat::execveat(dirfd, pathname, argv, &envp, 0)
        .change_context(ChildProcessError::ExecFailed)?;
    Ok(())
}

pub fn search_path(bin: &ShortCStr) -> Result<LocalFd, Report<ChildProcessError>> {
    let path_str = sys::env::getenv(c"PATH").unwrap_or(c"/usr/local/bin:/usr/bin:/bin".into());
    let slash: ShortCStr = c"/".into();
    for dir in path_str.split(b':') {
        if dir.is_empty() {
            continue;
        }
        let pathname =
            ShortCStr::concat(&[&dir, &slash, bin]).change_context(ChildProcessError::Never)?;
        if let Ok(fd) = sys::openat2::open(pathname.export(), O_PATH) {
            return Ok(fd);
        }
    }
    bail!(ChildProcessError::NotFound(bin.clone()))
}

pub fn resolve_path(bin: &ShortCStr) -> Result<LocalFd, Report<ChildProcessError>> {
    if bin.contains(b'/') {
        sys::openat2::open(bin.export(), O_PATH)
            .change_context_lazy(|| ChildProcessError::NotFound(bin.clone()))
    } else {
        search_path(bin)
    }
}

#[cfg(test)]
mod tests;

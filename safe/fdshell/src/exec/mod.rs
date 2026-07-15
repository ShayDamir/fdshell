mod environ;

use alloc::ffi::CString;
use alloc::format;
use alloc::vec::Vec;
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
    exports: &HashMap<ShortCStr, Vec<u8>>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let pid = sys::env::getpid();
    let cookie = format!("{}", pid);
    let exec_sock = shell_sock
        .map(|s| s.export())
        .transpose()
        .change_context(ChildProcessError::ExportFailed)?;
    let envp = get_environ(cookie.as_bytes(), exports, env_filter, exec_sock.as_ref());
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
    exports: &HashMap<ShortCStr, Vec<u8>>,
    env_filter: &EnvFilter,
    shell_sock: Option<&LocalFd>,
) -> Result<(), Report<ChildProcessError>> {
    let pid = sys::env::getpid();
    let cookie = format!("{}", pid);
    let exec_sock = shell_sock
        .map(|s| s.export())
        .transpose()
        .change_context(ChildProcessError::ExportFailed)?;
    let envp = get_environ(cookie.as_bytes(), exports, env_filter, exec_sock.as_ref());
    sys::execveat::execveat(dirfd, pathname, argv, &envp, 0)
        .change_context(ChildProcessError::ExecFailed)?;
    Ok(())
}

fn name_from_cstr(bin: &CStr) -> ShortCStr {
    ShortCStr::from_vec(bin.to_bytes().to_vec()).unwrap_or_else(|_| ShortCStr::new())
}

pub fn search_path(bin: &CStr) -> Result<LocalFd, Report<ChildProcessError>> {
    let path = sys::env::getenv(b"PATH").unwrap_or(b"/usr/local/bin:/usr/bin:/bin".to_vec());
    let bin_name = name_from_cstr(bin);
    for dir in path.split(|&b| b == b':').filter(|d| !d.is_empty()) {
        let full = [dir, b"/", bin.to_bytes()].concat();
        let pathname = CString::new(full).change_context(ChildProcessError::Never)?;
        if let Ok(fd) = sys::openat2::open(&pathname, O_PATH) {
            return Ok(fd);
        }
    }
    bail!(ChildProcessError::NotFound(bin_name))
}

pub fn resolve_path(bin: &CStr) -> Result<LocalFd, Report<ChildProcessError>> {
    if bin.to_bytes().contains(&b'/') {
        let bin_name = name_from_cstr(bin);
        sys::openat2::open(bin, O_PATH).change_context(ChildProcessError::NotFound(bin_name))
    } else {
        search_path(bin)
    }
}

#[cfg(test)]
mod tests;

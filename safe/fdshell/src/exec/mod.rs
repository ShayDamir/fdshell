mod environ;

use std::ffi::{CStr, CString};

use error_stack::{Report, ResultExt};
use sys::execveat::AT_EMPTY_PATH;
use sys::fcntl::O_PATH;
use sys::{AtFd, LocalFd, ShortCStr};

use crate::error::exec::ExecError;
use environ::get_environ;

pub fn exec_fd(
    fd: &LocalFd,
    argv: &[&CStr],
    exports: &[(ShortCStr, Vec<u8>)],
) -> Result<(), Report<ExecError>> {
    let pid = std::process::id();
    let cookie = format!("{}", pid);
    let envp = get_environ(cookie.as_bytes(), exports);
    let script_fd = fd.export().change_context(ExecError::ExportFailed)?;
    sys::execveat::execveat(script_fd.at(), c"", argv, &envp, AT_EMPTY_PATH)
        .change_context(ExecError::ExecFailed)?;
    Ok(())
}

pub fn exec_at(
    dirfd: AtFd<'_>,
    pathname: &CStr,
    argv: &[&CStr],
    exports: &[(ShortCStr, Vec<u8>)],
) -> Result<(), Report<ExecError>> {
    let pid = std::process::id();
    let cookie = format!("{}", pid);
    let envp = get_environ(cookie.as_bytes(), exports);
    sys::execveat::execveat(dirfd, pathname, argv, &envp, 0)
        .change_context(ExecError::ExecFailed)?;
    Ok(())
}

fn name_from_cstr(bin: &CStr) -> ShortCStr {
    ShortCStr::from_vec(bin.to_bytes().to_vec()).unwrap_or_else(|_| ShortCStr::new())
}

pub fn search_path(bin: &CStr) -> Result<LocalFd, Report<ExecError>> {
    let path = match std::env::var("PATH") {
        Ok(p) if !p.is_empty() => p,
        _ => "/usr/local/bin:/usr/bin:/bin".to_string(),
    };
    let bin_name = name_from_cstr(bin);
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let full = [dir.as_bytes(), b"/", bin.to_bytes()].concat();
        let pathname = match CString::new(full) {
            Ok(p) => p,
            Err(_) => return Err(Report::new(ExecError::NotFound).attach(bin_name)),
        };
        if let Ok(fd) = sys::openat2::open(&pathname, O_PATH) {
            return Ok(fd);
        }
    }
    Err(Report::new(ExecError::NotFound).attach(bin_name))
}

pub fn resolve_path(bin: &CStr) -> Result<LocalFd, Report<ExecError>> {
    if bin.to_bytes().contains(&b'/') {
        let bin_name = name_from_cstr(bin);
        sys::openat2::open(bin, O_PATH)
            .change_context(ExecError::NotFound)
            .attach(bin_name)
    } else {
        search_path(bin)
    }
}

#[cfg(test)]
mod tests;

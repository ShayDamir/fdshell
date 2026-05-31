#![forbid(unsafe_code)]

use std::ffi::{CStr, CString};
use sys::execveat::AT_EMPTY_PATH;
use sys::fcntl::O_PATH;
use sys::{AtFd, LocalFd};

pub fn exec_fd(fd: &LocalFd, argv: &[&CStr]) -> Result<(), i32> {
    let pid = std::process::id();
    let cookie = format!("{}", pid);
    let envp = get_environ(cookie.as_bytes());
    // dup to non-CLOEXEC so the kernel can pass /dev/fd/N to a script interpreter
    let script_fd = fd.export()?;
    sys::execveat::execveat(script_fd.at(), c"", argv, &envp, AT_EMPTY_PATH)
}

pub fn exec_at(dirfd: AtFd<'_>, pathname: &CStr, argv: &[&CStr]) -> Result<(), i32> {
    let pid = std::process::id();
    let cookie = format!("{}", pid);
    let envp = get_environ(cookie.as_bytes());
    sys::execveat::execveat(dirfd, pathname, argv, &envp, 0)
}

pub fn search_path(bin: &CStr) -> Result<LocalFd, i32> {
    let path = match std::env::var("PATH") {
        Ok(p) if !p.is_empty() => p,
        _ => "/usr/local/bin:/usr/bin:/bin".to_string(),
    };
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let full = [dir.as_bytes(), b"/", bin.to_bytes()].concat();
        let pathname = CString::new(full).map_err(|_| sys::errno::EINVAL)?;
        if let Ok(fd) = sys::openat2::open(&pathname, O_PATH) {
            return Ok(fd);
        }
    }
    Err(sys::errno::ENOENT)
}

pub fn resolve_path(bin: &CStr) -> Result<LocalFd, i32> {
    if bin.to_bytes().contains(&b'/') {
        sys::openat2::open(bin, O_PATH)
    } else {
        search_path(bin)
    }
}

fn get_environ(cookie: &[u8]) -> Vec<CString> {
    let env_iter = std::env::vars()
        .filter(|(k, _)| k != "FDSHELL_CAPTURE")
        .filter_map(|(k, v)| CString::new(format!("{k}={v}")).ok());
    if sys::shellfd::capture_active() {
        let entry = [b"FDSHELL_CAPTURE=", cookie].concat();
        env_iter.chain(CString::new(entry).ok()).collect()
    } else {
        env_iter.collect()
    }
}

#[cfg(test)]
mod tests;

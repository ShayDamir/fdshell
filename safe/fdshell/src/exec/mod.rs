#![forbid(unsafe_code)]

use std::ffi::{CStr, CString};
use sys::execveat::AT_EMPTY_PATH;
use sys::fcntl::{O_CLOEXEC, O_PATH};
use sys::openat2::OpenHow;
use sys::{AtFd, Fd};

pub fn exec_fd(fd: &Fd, argv: &[&CStr]) -> Result<(), i32> {
    let pid = std::process::id();
    let cookie = format!("{}", pid);
    let envp = get_environ(cookie.as_bytes());
    // dup to non-CLOEXEC so the kernel can pass /dev/fd/N to a script interpreter
    let script_fd = fd.dup()?;
    sys::execveat::execveat(script_fd.at(), c"", argv, &envp, AT_EMPTY_PATH)
}

pub fn search_path(bin: &CStr) -> Result<Fd, i32> {
    let path = match std::env::var("PATH") {
        Ok(p) if !p.is_empty() => p,
        _ => "/usr/local/bin:/usr/bin:/bin".to_string(),
    };
    let how = OpenHow {
        flags: O_PATH as u64 | O_CLOEXEC as u64,
        mode: 0,
        resolve: 0,
    };
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let full = [dir.as_bytes(), b"/", bin.to_bytes()].concat();
        let pathname = CString::new(full).map_err(|_| sys::errno::EINVAL)?;
        if let Ok(fd) = sys::openat2::openat2(AtFd::cwd(), &pathname, &how) {
            return Ok(fd);
        }
    }
    Err(sys::errno::ENOENT)
}

pub fn resolve_path(bin: &CStr) -> Result<Fd, i32> {
    if bin.to_bytes().contains(&b'/') {
        let how = OpenHow {
            flags: O_PATH as u64 | O_CLOEXEC as u64,
            mode: 0,
            resolve: 0,
        };
        sys::openat2::openat2(AtFd::cwd(), bin, &how)
    } else {
        search_path(bin)
    }
}

fn get_environ(cookie: &[u8]) -> Vec<CString> {
    let mut env: Vec<CString> = std::env::vars()
        .filter(|(k, _)| k != "FDSHELL_CAPTURE")
        .filter_map(|(k, v)| {
            let mut entry = k;
            entry.push('=');
            entry.push_str(&v);
            CString::new(entry).ok()
        })
        .collect();
    if sys::shellfd::capture_active() {
        let mut entry = b"FDSHELL_CAPTURE=".to_vec();
        entry.extend_from_slice(cookie);
        if let Ok(cs) = CString::new(entry) {
            env.push(cs);
        }
    }
    env
}

#[cfg(test)]
mod tests;

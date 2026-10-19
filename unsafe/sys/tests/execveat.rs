#![allow(clippy::unwrap_used)]

use core::ffi::CStr;
use std::ffi::CString;
use std::ops::Deref;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use sys::execveat::AT_EMPTY_PATH;

/// Path to the test helper binary that exits with 42.
const EXECVEAT_OK: &str = env!("CARGO_BIN_EXE_execveat_ok");

#[test]
fn execveat_noent() {
    let empty: &[&CStr] = &[];
    let result = sys::execveat::execveat(sys::AtFd::cwd(), c"/does/not/exist", empty, empty, 0);
    assert_eq!(result, Err(sys::SyscallError::ENOENT("unknown")));
}

#[test]
fn execveat_ok_path() {
    let exe = CString::new(EXECVEAT_OK).unwrap();
    let argv: &[&CStr] = &[c"execveat_ok"];
    let envp: &[&CStr] = &[];
    // SAFETY: fork+waitpid are standard POSIX operations; child
    // calls execveat which either replaces the process or _exit.
    unsafe {
        match libc::fork() {
            0 => {
                let _ = sys::execveat::execveat(sys::AtFd::cwd(), &exe, argv, envp, 0);
                libc::_exit(1);
            }
            -1 => panic!("fork failed"),
            pid => {
                let mut status = 0;
                libc::waitpid(pid, &mut status, 0);
                assert!(libc::WIFEXITED(status));
                assert_eq!(libc::WEXITSTATUS(status), 42);
            }
        }
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it runs
/// on a failing test's unwind, which is exactly the test that used to leak.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn test_dir() -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-execveat-test-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// Helper: creates a script with shebang, opens it, forks, runs execveat(fd, "", AT_EMPTY_PATH).
/// `cloexec` controls whether O_CLOEXEC is set on the fd.
/// `expect_ok` is whether execveat is expected to succeed (child exits 42) or fail (child exits 1).
fn execveat_script(script: &[u8], cloexec: bool, expect_ok: bool) {
    let dir = test_dir();
    let script_path = dir.join("script.sh");
    std::fs::write(&script_path, script).unwrap();
    {
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }
    let script_path_s = CString::new(script_path.to_str().unwrap()).unwrap();

    let flags = if cloexec {
        libc::O_RDONLY | libc::O_CLOEXEC
    } else {
        libc::O_RDONLY
    };
    // SAFETY: no data races; script_path_s is a valid C string.
    let raw = unsafe { libc::open(script_path_s.as_ptr(), flags) };
    assert!(raw >= 0, "open failed");

    // SAFETY: fork and waitpid are standard POSIX operations.
    unsafe {
        match libc::fork() {
            0 => {
                let dirfd = sys::AtFd::from_raw(raw);
                let argv: &[&CStr] = &[c"script.sh"];
                let envp: &[&CStr] = &[];
                let ret = sys::execveat::execveat(dirfd, c"", argv, envp, AT_EMPTY_PATH);
                libc::_exit(if ret.is_ok() == expect_ok { 42 } else { 1 });
            }
            -1 => panic!("fork failed"),
            pid => {
                let mut status = 0;
                libc::waitpid(pid, &mut status, 0);
                assert!(libc::WIFEXITED(status));
                assert_eq!(libc::WEXITSTATUS(status), 42);
            }
        }
    }
}

#[test]
fn execveat_script_ok() {
    execveat_script(b"#!/bin/sh\nexit 42\n", false, true);
}

#[test]
fn execveat_script_cloexec_fails() {
    execveat_script(b"#!/bin/sh\nexit 42\n", true, false);
}

#[test]
fn execveat_ok_fd() {
    let file = std::fs::File::open(EXECVEAT_OK).unwrap();
    let raw = file.as_raw_fd();
    let argv: &[&CStr] = &[c"execveat_ok"];
    let envp: &[&CStr] = &[];
    // SAFETY: fork+waitpid are standard POSIX operations; raw is
    // the fd of EXECVEAT_OK, opened before fork.
    unsafe {
        match libc::fork() {
            0 => {
                let dirfd = sys::AtFd::from_raw(raw);
                let _ = sys::execveat::execveat(dirfd, c"", argv, envp, AT_EMPTY_PATH);
                libc::_exit(1);
            }
            -1 => panic!("fork failed"),
            pid => {
                let mut status = 0;
                libc::waitpid(pid, &mut status, 0);
                assert!(libc::WIFEXITED(status));
                assert_eq!(libc::WEXITSTATUS(status), 42);
            }
        }
    }
}

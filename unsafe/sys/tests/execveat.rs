#![allow(clippy::unwrap_used)]

use core::ffi::CStr;
use std::ffi::CString;
use std::os::fd::AsRawFd;
use sys::execveat::AT_EMPTY_PATH;

/// Path to the test helper binary that exits with 42.
const EXECVEAT_OK: &str = env!("CARGO_BIN_EXE_execveat_ok");

#[test]
fn execveat_noent() {
    let empty: &[&CStr] = &[];
    let result = sys::execveat::execveat(sys::AtFd::cwd(), c"/does/not/exist", empty, empty, 0);
    assert_eq!(result, Err(libc::ENOENT));
}

#[test]
fn execveat_ok_path() {
    let exe = CString::new(EXECVEAT_OK).unwrap();
    let argv: &[&CStr] = &[c"execveat_ok"];
    let envp: &[&CStr] = &[];
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

#[test]
fn execveat_ok_fd() {
    let file = std::fs::File::open(EXECVEAT_OK).unwrap();
    let raw = file.as_raw_fd();
    let argv: &[&CStr] = &[c"execveat_ok"];
    let envp: &[&CStr] = &[];
    unsafe {
        match libc::fork() {
            0 => {
                // SAFETY: raw is the fd of EXECVEAT_OK, opened before fork.
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

#![allow(clippy::unwrap_used)]

use super::{exec_fd, resolve_path};
use crate::error::child_process::ChildProcessError;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::AtomicU64;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

const HELPER: &str = env!("EXEC_OK_PATH");

fn test_dir() -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-exec-test-{}-{}", std::process::id(), c))
}

fn setup(dir: &std::path::Path) -> CString {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let helper = dir.join("mybin");
    std::fs::copy(HELPER, &helper).unwrap();
    CString::new(helper.to_str().unwrap()).unwrap()
}

fn teardown(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn exec_child(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            f();
            std::process::exit(1);
        }
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

#[test]
fn resolve_path_finds_absolute() {
    let dir = test_dir();
    let abs = setup(&dir);
    let fd = resolve_path(&abs).unwrap();
    fd.verify().unwrap();
    teardown(&dir);
}

#[test]
fn resolve_path_finds_dot_slash() {
    let dir = test_dir();
    setup(&dir);
    let old = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let fd = resolve_path(c"./mybin").unwrap();
    fd.verify().unwrap();
    std::env::set_current_dir(&old).unwrap();
    teardown(&dir);
}

// -- get_environ tests (no fork) --

#[test]
fn exec_fd_with_exports() {
    let dir = test_dir();
    let abs = setup(&dir);
    let fd = resolve_path(&abs).unwrap();

    let mut exports_map = HashMap::new();
    exports_map.insert(ShortCStr::from(c"EXPORTED_VAR"), b"hello_world".to_vec());
    let exports: Vec<(sys::ShortCStr, Vec<u8>)> = exports_map.into_iter().collect();
    exec_child(
        || match exec_fd(&fd, &[&abs], &exports, &Default::default()) {
            Ok(()) => {}
            Err(report) => std::process::exit(report.current_context().exit_code()),
        },
    );

    teardown(&dir);
}

#[test]
fn resolve_path_missing_absolute() {
    let dir = test_dir();
    setup(&dir);
    let report = match resolve_path(c"/nonexistent-xxxxxxxx/binary") {
        Err(report) => report,
        Ok(_) => panic!("expected Err"),
    };
    assert!(matches!(
        report.current_context(),
        ChildProcessError::NotFound(_)
    ));
    teardown(&dir);
}

#[test]
fn resolve_path_missing_dot_slash() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let old = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let report = match resolve_path(c"./nope-xxxxxxxx") {
        Err(report) => report,
        Ok(_) => panic!("expected Err"),
    };
    assert!(matches!(
        report.current_context(),
        ChildProcessError::NotFound(_)
    ));
    std::env::set_current_dir(&old).unwrap();
    teardown(&dir);
}

// -- exec tests (fork + exec_fd) -- sequential in one test to avoid races

#[test]
fn exec_with_paths() {
    let dir = test_dir();
    let abs = setup(&dir);

    exec_child(|| {
        let fd = resolve_path(&abs).unwrap();
        match exec_fd(&fd, &[&abs], &[], &Default::default()) {
            Ok(()) => {}
            Err(report) => std::process::exit(report.current_context().exit_code()),
        }
    });

    let cd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    exec_child(|| {
        let fd = resolve_path(c"./mybin").unwrap();
        match exec_fd(&fd, &[c"mybin"], &[], &Default::default()) {
            Ok(()) => {}
            Err(report) => std::process::exit(report.current_context().exit_code()),
        }
    });
    std::env::set_current_dir(&cd).unwrap();

    teardown(&dir);
}

#[test]
fn exec_script_via_resolve_fd() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let script_path = dir.join("script.sh");
    std::fs::write(&script_path, b"#!/bin/sh\nexit 42\n").unwrap();
    {
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }
    let script_cs = CString::new(script_path.to_str().unwrap()).unwrap();

    exec_child(|| {
        let fd = resolve_path(&script_cs).unwrap();
        match exec_fd(&fd, &[&script_cs], &[], &Default::default()) {
            Ok(()) => {}
            Err(report) => std::process::exit(report.current_context().exit_code()),
        }
    });

    teardown(&dir);
}

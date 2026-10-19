#![allow(clippy::unwrap_used)]

use super::{exec_fd, resolve_path};
use crate::error::child_process::ChildProcessError;
use alloc::format;
use hashbrown::HashMap;
use std::ffi::CString;
use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use sys::ImportedStr;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

const HELPER: &str = env!("EXEC_OK_PATH");

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

fn scratch() -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-exec-test-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

fn setup(dir: &std::path::Path) -> CString {
    let helper = dir.join("mybin");
    std::fs::copy(HELPER, &helper).unwrap();
    CString::new(helper.to_str().unwrap()).unwrap()
}

fn exec_child(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            f();
            sys::exit(1);
        }
        Some(pidfd) => {
            let status = pidfd.wait_pidfd().unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

#[test]
fn resolve_path_finds_absolute() {
    let dir = scratch();
    let abs = setup(&dir);
    let mut bin = ShortCStr::new();
    bin.push(abs.as_ref());
    let fd = resolve_path(&bin, &HashMap::new(), false, None).unwrap();
    fd.verify().unwrap();
}

#[test]
fn resolve_path_finds_dot_slash() {
    let dir = scratch();
    setup(&dir);
    let old = std::env::current_dir().unwrap();
    std::env::set_current_dir(&*dir).unwrap();
    let fd = resolve_path(&c"./mybin".into(), &HashMap::new(), false, None).unwrap();
    fd.verify().unwrap();
    std::env::set_current_dir(&old).unwrap();
}

// -- get_environ tests (no fork) --

#[test]
fn exec_fd_with_exports() {
    let dir = scratch();
    let abs = setup(&dir);
    let mut bin = ShortCStr::new();
    bin.push(abs.as_ref());
    let fd = resolve_path(&bin, &HashMap::new(), false, None).unwrap();

    let mut exports_map = HashMap::new();
    exports_map.insert(
        c"EXPORTED_VAR".into(),
        ImportedStr::shell(c"hello_world".into()),
    );
    exec_child(
        || match exec_fd(&fd, &[&abs], &[], &exports_map, &Default::default(), None) {
            Ok(()) => {}
            Err(report) => sys::exit(report.current_context().exit_code()),
        },
    );
}

#[test]
fn resolve_path_missing_absolute() {
    let dir = scratch();
    setup(&dir);
    let report = match resolve_path(
        &c"/nonexistent-xxxxxxxx/binary".into(),
        &HashMap::new(),
        false,
        None,
    ) {
        Err(report) => report,
        Ok(_) => panic!("expected Err"),
    };
    assert!(matches!(
        report.current_context(),
        ChildProcessError::NotFound(_)
    ));
}

#[test]
fn resolve_path_missing_dot_slash() {
    let dir = scratch();
    let old = std::env::current_dir().unwrap();
    std::env::set_current_dir(&*dir).unwrap();
    let report = match resolve_path(&c"./nope-xxxxxxxx".into(), &HashMap::new(), false, None) {
        Err(report) => report,
        Ok(_) => panic!("expected Err"),
    };
    assert!(matches!(
        report.current_context(),
        ChildProcessError::NotFound(_)
    ));
    std::env::set_current_dir(&old).unwrap();
}

#[test]
fn resolve_path_empty_name_is_not_found() {
    // A bare PATH directory is openable, so an empty name must not resolve to
    // one; it fails like any unknown command.
    let report = match resolve_path(&ShortCStr::new(), &HashMap::new(), false, None) {
        Err(report) => report,
        Ok(_) => panic!("expected Err"),
    };
    assert!(matches!(
        report.current_context(),
        ChildProcessError::NotFound(_)
    ));
}

// -- exec tests (fork + exec_fd) -- sequential in one test to avoid races

#[test]
fn exec_with_paths() {
    let dir = scratch();
    let abs = setup(&dir);

    exec_child(|| {
        let mut bin = ShortCStr::new();
        bin.push(abs.as_ref());
        let fd = resolve_path(&bin, &HashMap::new(), false, None).unwrap();
        match exec_fd(
            &fd,
            &[&abs],
            &[],
            &HashMap::new(),
            &Default::default(),
            None,
        ) {
            Ok(()) => {}
            Err(report) => sys::exit(report.current_context().exit_code()),
        }
    });

    let cd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&*dir).unwrap();
    exec_child(|| {
        let fd = resolve_path(&c"./mybin".into(), &HashMap::new(), false, None).unwrap();
        match exec_fd(
            &fd,
            &[c"mybin"],
            &[],
            &HashMap::new(),
            &Default::default(),
            None,
        ) {
            Ok(()) => {}
            Err(report) => sys::exit(report.current_context().exit_code()),
        }
    });
    std::env::set_current_dir(&cd).unwrap();
}

#[test]
fn exec_script_via_resolve_fd() {
    let dir = scratch();
    let script_path = dir.join("script.sh");
    std::fs::write(&script_path, b"#!/bin/sh\nexit 42\n").unwrap();
    {
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }
    let script_cs = CString::new(script_path.to_str().unwrap()).unwrap();

    exec_child(|| {
        let mut script_bin = ShortCStr::new();
        script_bin.push(script_cs.as_ref());
        let fd = resolve_path(&script_bin, &HashMap::new(), false, None).unwrap();
        match exec_fd(
            &fd,
            &[&script_cs],
            &[],
            &HashMap::new(),
            &Default::default(),
            None,
        ) {
            Ok(()) => {}
            Err(report) => sys::exit(report.current_context().exit_code()),
        }
    });
}

#[test]
fn resolve_path_hash_pin_hit_without_path_entry() {
    let dir = scratch();
    let abs = setup(&dir);
    let mut name = ShortCStr::new();
    name.push(c"mybin");
    let mut path = ShortCStr::new();
    path.push(abs.as_ref());
    let mut table = HashMap::new();
    table.insert(name.clone(), path);
    // No `mybin` on PATH: resolution succeeds only through the pin.
    let report = match resolve_path(&name, &HashMap::new(), false, None) {
        Err(report) => report,
        Ok(_) => panic!("without a table entry the bare name must not resolve"),
    };
    assert!(matches!(
        report.current_context(),
        ChildProcessError::NotFound(_)
    ));
    let fd = resolve_path(&name, &table, false, None).unwrap();
    fd.verify().unwrap();
}

#[test]
fn resolve_path_hash_bypassed_for_paths() {
    let dir = scratch();
    let abs = setup(&dir);
    let mut name = ShortCStr::new();
    name.push(abs.as_ref());
    let mut table = HashMap::new();
    let mut junk = ShortCStr::new();
    junk.push(c"/nonexistent-hash-pin");
    table.insert(name.clone(), junk);
    // A name containing `/` bypasses the table: the pin must be ignored.
    let fd = resolve_path(&name, &table, false, None).unwrap();
    fd.verify().unwrap();
}

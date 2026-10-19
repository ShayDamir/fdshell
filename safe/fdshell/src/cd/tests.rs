#![cfg_attr(test, allow(clippy::unwrap_used))]
use super::*;
use crate::state::{FdVar, ShellState};
use alloc::format;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use sys::siginfo::WaitStatus;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. A *fixed* name
/// (`fdshell-cd-test`) is shared by every concurrent suite run with no pid
/// recycling at all, and a dir that outlives its process is itself the
/// collision: pids get recycled, and a later run inherits the earlier run's
/// dir with that run's files in it (LESSONS, tasks #255/#318; sweep #342).
/// Removal is `.ok()`-tolerant: teardown never panics, and it runs on a
/// failing test's unwind, which is exactly the test that used to leak.
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

/// A pid + counter scratch dir named `fdshell-cd-<tag>-<pid>-<counter>`,
/// removed when the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-cd-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

fn pos() -> Position {
    Position::new(1, 1)
}

fn child_test(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            f();
            sys::exit(42);
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
fn cd_to_tmp() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state, pos()).unwrap();
        let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        cwd.fd.verify().unwrap();
    });
}

#[test]
fn cd_to_home() {
    let home = std::env::var_os("HOME");
    let Some(home_path) = home else {
        child_test(|| {
            let mut state = ShellState::new();
            let e = cd(&[], &mut state, pos()).unwrap_err();
            assert!(matches!(e.current_context(), CdError::HomeNotSet));
        });
        return;
    };
    if std::path::Path::new(&home_path).exists() {
        child_test(|| {
            let mut state = ShellState::new();
            cd(&[], &mut state, pos()).unwrap();
            let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
            cwd.fd.verify().unwrap();
        });
    } else {
        child_test(|| {
            let mut state = ShellState::new();
            let e = cd(&[], &mut state, pos()).unwrap_err();
            assert!(matches!(e.current_context(), CdError::CdPathOpen));
        });
    }
}

#[test]
fn cd_to_self() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state, pos()).unwrap();
        let src = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        let cwd_fd = src.fd.try_clone().unwrap();
        let trace = src.trace.clone();
        state.fds.insert(c"CWD".into(), FdVar { fd: cwd_fd, trace });
        cd(&[c"%CWD".into()], &mut state, pos()).unwrap();
        let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        cwd.fd.verify().unwrap();
    });
}

#[test]
fn cd_missing_path() {
    child_test(|| {
        let mut state = ShellState::new();
        let bad = c"/nonexistent-cd-test-xxxxxxxx".into();
        let e = cd(&[bad], &mut state, pos()).unwrap_err();
        assert!(matches!(e.current_context(), CdError::CdPathOpen));
    });
}

#[test]
fn cd_missing_var() {
    child_test(|| {
        let mut state = ShellState::new();
        let bad = c"%NONEXISTENT".into();
        let e = cd(&[bad], &mut state, pos()).unwrap_err();
        assert!(matches!(e.current_context(), CdError::FdNotSet));
    });
}

#[test]
fn cd_dash_switches_to_oldpwd() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state, pos()).unwrap();
        let root = c"/".into();
        cd(&[root], &mut state, pos()).unwrap();
        let dash = c"-".into();
        cd(&[dash], &mut state, pos()).unwrap();
        let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        cwd.fd.verify().unwrap();
        let old = state.fds.get::<sys::ShortCStr>(&c"OLDCWD".into()).unwrap();
        old.fd.verify().unwrap();
    });
}

#[test]
fn cd_move_cwd_to_oldcwd() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut state, pos()).unwrap();
        assert!(state.fds.contains_key::<sys::ShortCStr>(&c"CWD".into()));
        assert!(!state.fds.contains_key::<sys::ShortCStr>(&c"OLDCWD".into()));
        let root = c"/".into();
        cd(&[root], &mut state, pos()).unwrap();
        assert!(state.fds.contains_key::<sys::ShortCStr>(&c"CWD".into()));
        assert!(state.fds.contains_key::<sys::ShortCStr>(&c"OLDCWD".into()));
    });
}

#[test]
fn cd_file_fails() {
    child_test(|| {
        let mut state = ShellState::new();
        let dir = scratch("test");
        let file_path = dir.join("testfile");
        std::fs::write(&file_path, b"hello\n").unwrap();

        cd(&[c"/tmp".into()], &mut state, pos()).unwrap();

        // Try to cd into a regular file — should fail because O_DIRECTORY
        let path_str = file_path.to_str().unwrap();
        let short_path = sys::ShortCStr::from_vec(path_str.as_bytes().to_vec()).unwrap();
        let e = cd(&[short_path], &mut state, pos()).unwrap_err();
        assert!(matches!(e.current_context(), CdError::CdPathOpen));
    });
}

#[test]
fn cd_symlink_to_dir_fails() {
    child_test(|| {
        let mut state = ShellState::new();
        let dir = scratch("symlink-test");
        std::fs::create_dir_all(dir.join("realdir")).unwrap();
        std::os::unix::fs::symlink(dir.join("realdir"), dir.join("link_to_dir")).unwrap();

        cd(&[c"/tmp".into()], &mut state, pos()).unwrap();

        // Try to cd into a symlink to a directory — should fail because O_NOFOLLOW
        let link_path = dir.join("link_to_dir");
        let path_str = link_path.to_str().unwrap();
        let short_path = sys::ShortCStr::from_vec(path_str.as_bytes().to_vec()).unwrap();
        let e = cd(&[short_path], &mut state, pos()).unwrap_err();
        assert!(matches!(e.current_context(), CdError::CdPathOpen));
    });
}

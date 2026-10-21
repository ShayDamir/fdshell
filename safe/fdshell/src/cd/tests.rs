#![cfg_attr(test, allow(clippy::unwrap_used))]
use super::*;
use crate::state::{FdVar, ShellState};
use alloc::format;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use sys::ImportedStr;
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

/// A path as the `ShortCStr` the target arms take.
fn sc(path: &std::path::Path) -> sys::ShortCStr {
    sys::ShortCStr::from_vec(path.to_str().unwrap().as_bytes().to_vec()).unwrap()
}

/// A scratch dir holding a `marker` file, so a bare `cd` can be verified by a
/// **relative** open of that name: it lands only in the directory the shell
/// variable named (the startup `HOME` carries no marker).
fn home_dir(tag: &str) -> Scratch {
    let dir = scratch(tag);
    std::fs::write(dir.join("marker"), b"x").unwrap();
    dir
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
        cd(CdTarget::Path(tmp), &mut state, pos()).unwrap();
        let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        cwd.fd.verify().unwrap();
    });
}

#[test]
fn cd_to_home() {
    // The **fallback** row: `HOME` in the inherited `environ` snapshot only
    // (the harness env), the shell's `strings` empty — the same lookup `$HOME`
    // and `~` use.
    let home = std::env::var_os("HOME");
    let Some(home_path) = home else {
        child_test(|| {
            let mut state = ShellState::new();
            let e = cd(CdTarget::Home, &mut state, pos()).unwrap_err();
            assert!(matches!(e.current_context(), CdError::HomeNotSet));
        });
        return;
    };
    if std::path::Path::new(&home_path).exists() {
        child_test(|| {
            let mut state = ShellState::new();
            cd(CdTarget::Home, &mut state, pos()).unwrap();
            let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
            cwd.fd.verify().unwrap();
        });
    } else {
        child_test(|| {
            let mut state = ShellState::new();
            let e = cd(CdTarget::Home, &mut state, pos()).unwrap_err();
            assert!(matches!(e.current_context(), CdError::CdPathOpen));
        });
    }
}

/// Task #287: the bare `cd` reads `HOME` through `ShellState::var_value` — the
/// shell's own variables first, the inherited `environ` snapshot as the
/// documented fallback — the same lookup `$HOME` and `~` use, **never** the live
/// process environment. Measured in bash 5.3.9 (`cd; pwd` after
/// `HOME=<scratch>` prints `<scratch>`, rc 0); the shell assignment and the
/// inherited row deliberately disagree here, so reading the environ first, or
/// the process environment, lands on `/tmp`, where the marker's relative open
/// fails.
#[test]
fn cd_home_prefers_the_shell_variable() {
    let dir = home_dir("home-var");
    child_test(|| {
        let mut state = ShellState::new();
        state.environ.clear();
        state.environ.push((c"HOME".into(), c"/tmp".into()));
        state.set_var(c"HOME".into(), ImportedStr::shell(sc(&dir)));
        cd(CdTarget::Home, &mut state, pos()).unwrap();
        // `cd` fchdir'd the child, so this lands only in the directory the
        // shell variable named.
        sys::openat2::open(c"marker", sys::fcntl::O_RDONLY).unwrap();
        // The provenance of the moved CWD still names the `HOME` fact.
        let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        assert_eq!(cwd.trace.origin, Origin::EnvVar(ShortCStr::from(c"HOME")));
    });
}

/// The fallback row with a real target: `HOME` named in no shell variable, only
/// in the inherited table, so the lookup must not stop at `strings`.
#[test]
fn cd_home_falls_back_to_the_inherited_environ() {
    let dir = home_dir("home-env");
    child_test(|| {
        let mut state = ShellState::new();
        state.environ.clear();
        state.environ.push((c"HOME".into(), sc(&dir)));
        cd(CdTarget::Home, &mut state, pos()).unwrap();
        sys::openat2::open(c"marker", sys::fcntl::O_RDONLY).unwrap();
    });
}

/// `HOME=""` is a **value**, not an unset: the target is the empty path, so the
/// failure is the open on `""` (`CdPathOpen`), never `HomeNotSet`. bash 5.3.9
/// makes the bare `cd` a silent no-op there (`HOME=; cd; pwd` → the cwd, rc 0),
/// the `cd ""` family; fdshell reports the open failure at rc 1.
///
/// Both failure rows run **in the test process**, not in a `child_test` fork: a
/// failed `cd` never reaches `fchdir`, so they move nothing, and the fork's
/// child exits on a raw `sys::exit`, which drops its gcov counters — forking
/// them would leave the two arms at 0 hits in `nix build .#coverage` (LESSONS:
/// "Integration tests contribute no coverage").
#[test]
fn cd_home_empty_is_the_open_failure() {
    let mut state = ShellState::new();
    // The inherited row points at a real directory: an empty shell value read
    // as "unset" would succeed there instead of failing the open.
    state.environ.clear();
    state.environ.push((c"HOME".into(), c"/tmp".into()));
    state.set_var(c"HOME".into(), ImportedStr::shell(ShortCStr::new()));
    let e = cd(CdTarget::Home, &mut state, pos()).unwrap_err();
    assert!(matches!(e.current_context(), CdError::CdPathOpen));
}

/// `HOME` in **neither** table is `HomeNotSet`. bash 5.3.9 prints
/// `cd: HOME not set` at rc 1 for the bare `cd` (measured — the
/// password-database login directory is `~`'s fallback, not `cd`'s), so the
/// report and the rc agree.
#[test]
fn cd_home_unset_reports_home_not_set() {
    let mut state = ShellState::new();
    // The harness env has a `HOME`, so clearing the snapshot is what makes this
    // row mean "set in neither table" (a live-environ lookup would find the
    // developer's home and succeed).
    state.environ.clear();
    let e = cd(CdTarget::Home, &mut state, pos()).unwrap_err();
    assert!(matches!(e.current_context(), CdError::HomeNotSet));
}

#[test]
fn cd_to_self() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(CdTarget::Path(tmp), &mut state, pos()).unwrap();
        let src = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        let cwd_fd = src.fd.try_clone().unwrap();
        let trace = src.trace.clone();
        state.fds.insert(c"CWD".into(), FdVar { fd: cwd_fd, trace });
        cd(CdTarget::Var(c"CWD".into()), &mut state, pos()).unwrap();
        let cwd = state.fds.get::<sys::ShortCStr>(&c"CWD".into()).unwrap();
        cwd.fd.verify().unwrap();
    });
}

#[test]
fn cd_missing_path() {
    child_test(|| {
        let mut state = ShellState::new();
        let bad = c"/nonexistent-cd-test-xxxxxxxx".into();
        let e = cd(CdTarget::Path(bad), &mut state, pos()).unwrap_err();
        assert!(matches!(e.current_context(), CdError::CdPathOpen));
    });
}

#[test]
fn cd_missing_var() {
    child_test(|| {
        let mut state = ShellState::new();
        let bad = c"NONEXISTENT".into();
        let e = cd(CdTarget::Var(bad), &mut state, pos()).unwrap_err();
        assert!(matches!(e.current_context(), CdError::FdNotSet));
    });
}

/// `Path("-")` opens a directory literally named `-`: the old-cwd form is the
/// caller's decision (`intercept/cd.rs` reads `-` off the **expanded** field),
/// so `cd` itself keeps no word rule that could fire on a `-` path.
#[test]
fn cd_path_dash_opens_the_literal_dir() {
    child_test(|| {
        let mut state = ShellState::new();
        let dir = scratch("dash-dir");
        std::fs::create_dir(dir.join("-")).unwrap();
        std::fs::write(dir.join("-/marker"), b"x").unwrap();
        let base = sys::ShortCStr::from_vec(dir.to_str().unwrap().as_bytes().to_vec()).unwrap();
        cd(CdTarget::Path(base), &mut state, pos()).unwrap();
        cd(CdTarget::Path(c"-".into()), &mut state, pos()).unwrap();
        // `cd` fchdir'd the process, so a relative open lands inside the `-`
        // directory only when the literal path form opened it.
        sys::openat2::open(c"marker", sys::fcntl::O_RDONLY).unwrap();
    });
}

#[test]
fn cd_dash_switches_to_oldpwd() {
    child_test(|| {
        let mut state = ShellState::new();
        let tmp = c"/tmp".into();
        cd(CdTarget::Path(tmp), &mut state, pos()).unwrap();
        let root = c"/".into();
        cd(CdTarget::Path(root), &mut state, pos()).unwrap();
        cd(CdTarget::OldCwd, &mut state, pos()).unwrap();
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
        cd(CdTarget::Path(tmp), &mut state, pos()).unwrap();
        assert!(state.fds.contains_key::<sys::ShortCStr>(&c"CWD".into()));
        assert!(!state.fds.contains_key::<sys::ShortCStr>(&c"OLDCWD".into()));
        let root = c"/".into();
        cd(CdTarget::Path(root), &mut state, pos()).unwrap();
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

        cd(CdTarget::Path(c"/tmp".into()), &mut state, pos()).unwrap();

        // Try to cd into a regular file — should fail because O_DIRECTORY
        let path_str = file_path.to_str().unwrap();
        let short_path = sys::ShortCStr::from_vec(path_str.as_bytes().to_vec()).unwrap();
        let e = cd(CdTarget::Path(short_path), &mut state, pos()).unwrap_err();
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

        cd(CdTarget::Path(c"/tmp".into()), &mut state, pos()).unwrap();

        // Try to cd into a symlink to a directory — should fail because O_NOFOLLOW
        let link_path = dir.join("link_to_dir");
        let path_str = link_path.to_str().unwrap();
        let short_path = sys::ShortCStr::from_vec(path_str.as_bytes().to_vec()).unwrap();
        let e = cd(CdTarget::Path(short_path), &mut state, pos()).unwrap_err();
        assert!(matches!(e.current_context(), CdError::CdPathOpen));
    });
}

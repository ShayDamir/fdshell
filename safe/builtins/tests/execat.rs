#![allow(clippy::unwrap_used)]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use sys::fcntl::{O_DIRECTORY, O_PATH};
use sys::siginfo::WaitStatus;

const EXEC_OK: &str = env!("CARGO_BIN_EXE_builtin_exec_ok");

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_parse_err(args: &[&str]) {
    with_args(args, |a| match builtins::execat::parse::execat_parse(a) {
        Err(e) => {
            let ctx = e.current_context();
            match ctx {
                BuiltinError::Help | BuiltinError::InvalidArgument(_) => {}
                other => panic!("unexpected error: {other}"),
            }
        }
        _ => panic!("expected Err"),
    });
}

fn assert_parse_ok<F: FnOnce(&builtins::execat::parse::ExecAtConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::execat::parse::execat_parse(a) {
        Ok(cfg) => f(&cfg),
        Err(e) => panic!("expected Ok, got Err({e})"),
    });
}

#[test]
fn empty_args() {
    assert_parse_err(&[]);
}

#[test]
fn help_long() {
    assert_parse_err(&["--help"]);
}

#[test]
fn help_with_other_args() {
    with_args(
        &["%CWD", "--help"],
        |a| match builtins::execat::parse::execat_parse(a) {
            Err(e) => {
                let ctx = e.current_context();
                assert!(
                    matches!(ctx, BuiltinError::Help),
                    "expected Help, got {ctx}"
                );
            }
            _ => panic!("expected Err"),
        },
    );
}

#[test]
fn missing_percent() {
    assert_parse_err(&["CWD", "bin"]);
}

#[test]
fn empty_var_name() {
    assert_parse_err(&["%", "bin"]);
}

#[test]
fn missing_pathname() {
    assert_parse_err(&["%CWD"]);
}

#[test]
fn empty_pathname() {
    assert_parse_err(&["%CWD", ""]);
}

#[test]
fn var_pathname_argv() {
    assert_parse_ok(&["%CWD", "bin/x", "a", "b"], |cfg| {
        assert_eq!(cfg.var.to_bytes(), b"%CWD");
        assert_eq!(cfg.pathname.to_bytes(), b"bin/x");
    });
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

fn scratch() -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-execat-test-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

fn open_test_dir(dir: &std::path::Path) -> sys::LocalFd {
    let cs = CString::new(dir.to_str().unwrap()).unwrap();
    sys::openat2::open(&cs, O_PATH + O_DIRECTORY).unwrap()
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
fn execat_exec_ok() {
    let dir = scratch();
    let target = dir.join("mybin");
    std::fs::copy(EXEC_OK, &target).unwrap();
    {
        let mut perms = std::fs::metadata(&target).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target, perms).unwrap();
    }
    let dirfd = open_test_dir(&dir);
    exec_child(|| {
        match builtins::execat::execat_exec(dirfd.at(), c"mybin", &[c"builtin_exec_ok"], &[]) {
            Ok(()) => {}
            Err(_) => sys::exit(1),
        }
    });
}

#[test]
fn execat_exec_missing_fails() {
    let dir = scratch();
    let dirfd = open_test_dir(&dir);
    exec_child(|| {
        let r = builtins::execat::execat_exec(dirfd.at(), c"nope-xxxxxxxx", &[c"x"], &[]);
        if r.is_ok() {
            sys::exit(1);
        } else {
            sys::exit(42);
        }
    });
}

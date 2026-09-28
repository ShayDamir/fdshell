#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir with two files, `a` and `b` (same size, different inodes);
/// each test gets its own dir (pid + counter: tests in one binary share the
/// pid); removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fdshell-openat2-sameas-{}-{}",
            std::process::id(),
            c
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a"), b"same").unwrap();
        std::fs::write(dir.join("b"), b"same").unwrap();
        Self(dir)
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn run(dir: &std::path::Path, script: &str) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    str::from_utf8(&out.stdout).unwrap().to_string()
}

fn stderr(out: &Output) -> String {
    str::from_utf8(&out.stderr).unwrap().to_string()
}

/// Same file: the check passes, the capture is committed, and both fds name
/// the same inode.
#[test]
fn same_as_match_commits_capture() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY a %>%ref; \
         builtin openat2 --same-as %ref --flags O_RDONLY a %>%f; \
         builtin echo rc=$?; \
         if test %f -fdeq %ref; then printf same; fi",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let out = stdout(&out);
    assert!(out.contains("rc=0"), "stdout={out:?}");
    assert!(out.contains("same"), "stdout={out:?}");
}

/// Different file (same size, different inode): the builtin fails and the
/// stderr carries the mismatch message.
#[test]
fn same_as_mismatch_fails() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY a %>%ref; \
         builtin openat2 --same-as %ref --flags O_RDONLY b; \
         builtin echo rc=$?",
    );
    let err = stderr(&out);
    let out = stdout(&out);
    assert!(out.contains("rc=1"), "stdout={out:?} stderr={err}");
    assert!(
        err.contains("same file as the --same-as reference fd"),
        "stderr={err:?}"
    );
}

/// Mismatch with a ` %>%f` capture: the capture is never committed. Captures
/// are only applied to a child that exited 0 (`postlaunch`), so the
/// incomplete-capture machinery does not run — the child's exit code
/// propagates, same as any failing delegated builtin (e.g. openat2 ENOENT).
#[test]
fn same_as_mismatch_with_capture_never_commits() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY a %>%ref; \
         builtin openat2 --same-as %ref --flags O_RDONLY b %>%f; \
         builtin echo rc=$?",
    );
    let err = stderr(&out);
    let out = stdout(&out);
    assert!(out.contains("rc=1"), "stdout={out:?} stderr={err}");
    assert!(
        err.contains("same file as the --same-as reference fd"),
        "stderr={err:?}"
    );
    assert!(!err.contains("incomplete capture"), "stderr={err:?}");
}

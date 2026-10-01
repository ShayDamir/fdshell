//! E2E: the `symlinkat` builtin — create a symbolic link.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with a file `f` and a `sub` dir; removed on drop.
struct Scratch(PathBuf);

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Scratch {
    fn new() -> Self {
        // Tests in one binary run on parallel threads (same pid), so the
        // dir must be unique per test, not per process.
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "fdshell-symlinkat-e2e-{}-{}",
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f"), b"file").unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
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

/// `symlinkat target link` creates the link; `readlink` prints the stored target.
#[test]
fn creates_link_and_readlink_prints_target() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin symlinkat f link; builtin readlink link");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "f\n");
}

/// The target is stored verbatim, even when it does not exist.
#[test]
fn stores_target_verbatim() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin symlinkat no/such/target link; builtin readlink link",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "no/such/target\n");
}

/// `--dirfd %d` creates the link under the pinned dir, not the CWD.
#[test]
fn dirfd_var_changes_base() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY sub %>%d; builtin symlinkat t l --dirfd %d; \
         builtin echo rc=$?",
    );
    assert!(stdout(&out).contains("rc=0"), "stdout={:?}", stdout(&out));
    assert!(
        std::fs::symlink_metadata(dir.join("sub").join("l")).is_ok(),
        "sub/l should exist"
    );
    assert!(
        std::fs::symlink_metadata(dir.join("l")).is_err(),
        "CWD l must not exist"
    );
}

/// Creating the same link twice: the second is `EEXIST` (17).
#[test]
fn create_twice_is_eexist() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin symlinkat f link; builtin symlinkat f link; builtin echo rc=$?",
    );
    assert!(stdout(&out).contains("rc=17"), "stdout={:?}", stdout(&out));
}

/// A missing parent dir is `ENOENT` (2).
#[test]
fn missing_parent_is_enoent() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin symlinkat f nope/link; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
}

/// A non-directory fd as `--dirfd` is `ENOTDIR` (20).
#[test]
fn nondirectory_dirfd_is_enotdir() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDWR f %>%h; \
         builtin symlinkat t l --dirfd %h; builtin echo rc=$?",
    );
    assert!(stdout(&out).contains("rc=20"), "stdout={:?}", stdout(&out));
}

/// Parse errors are exit 1 with a report on stderr; no args and `--help` exit 0.
#[test]
fn parse_errors() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin symlinkat --bad t l");
    assert_eq!(out.status.code(), Some(1));
    assert!(!stderr(&out).is_empty(), "stderr should carry the report");

    let out = run(&dir, "builtin symlinkat");
    assert_eq!(out.status.code(), Some(0));
    assert!(
        stdout(&out).is_empty(),
        "Help prints nothing: {:?}",
        stdout(&out)
    );
}

/// `--help` exits 0.
#[test]
fn help_flag_exits_zero() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin symlinkat --help");
    assert_eq!(out.status.code(), Some(0));
}

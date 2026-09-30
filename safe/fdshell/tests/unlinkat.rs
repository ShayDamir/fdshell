//! E2E: the `unlinkat` builtin — remove a file or directory entry.
//!
//! The motivating case is the AF_UNIX filesystem socket path left behind by
//! `bind`: the shell never auto-unlinks it, so the script must `unlinkat` it
//! (on startup to clear a stale path, on shutdown to clean up).

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with a file `f` and a `sub` dir holding `g`; removed on drop.
struct Scratch(PathBuf);

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Scratch {
    fn new() -> Self {
        // Tests in one binary run on parallel threads (same pid), so the
        // dir must be unique per test, not per process.
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("fdshell-unlinkat-e2e-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f"), b"file").unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("g"), b"subfile").unwrap();
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

/// `unlinkat f` removes the file from the CWD; `ls` no longer lists it.
#[test]
fn unlinks_file_in_cwd() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin unlinkat f; builtin ls .");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let listing = stdout(&out);
    assert!(
        !listing.lines().any(|l| l == "f"),
        "f should be gone: {listing:?}"
    );
    assert!(
        listing.lines().any(|l| l == "sub"),
        "sub should remain: {listing:?}"
    );
}

/// Unlinking the same path twice: the second is `ENOENT` (2).
#[test]
fn unlink_twice_is_enoent() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin unlinkat f; builtin unlinkat f; builtin echo rc=$?",
    );
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
}

/// `--dirfd %d` resolves the entry against the dirfd, not the CWD: `sub/g` is
/// removed while a CWD `g` (absent here) would be untouched — a `--dirfd`-less
/// `unlinkat g` is `ENOENT` (2).
#[test]
fn dirfd_var_changes_base() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY sub %>%d; builtin unlinkat --dirfd %d g; \
         builtin echo rc=$?",
    );
    assert!(stdout(&out).contains("rc=0"), "stdout={:?}", stdout(&out));
    assert!(!dir.join("sub").join("g").exists(), "sub/g should be gone");
    // No top-level `g` exists, so the CWD-relative form is ENOENT.
    let out = run(&dir, "builtin unlinkat g; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
}

/// `--flags AT_REMOVEDIR` removes a directory.
#[test]
fn removedir_flag_removes_directory() {
    let dir = Scratch::new();
    // `sub` holds `g`; remove the entry first (AT_REMOVEDIR needs an empty dir),
    // then the directory itself.
    let out = run(
        &dir,
        "builtin unlinkat sub/g; builtin unlinkat --flags AT_REMOVEDIR sub; \
         builtin echo rc=$?",
    );
    assert!(stdout(&out).contains("rc=0"), "stdout={:?}", stdout(&out));
    assert!(!dir.join("sub").exists(), "sub should be gone");
}

/// Unlinking a directory without `AT_REMOVEDIR` is `EISDIR` (21).
#[test]
fn directory_without_flag_is_eisdir() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin unlinkat sub; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=21"), "stdout={:?}", stdout(&out));
}

/// A non-directory fd as `--dirfd` is `ENOTDIR` (20).
#[test]
fn nondirectory_dirfd_is_enotdir() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDWR f %>%h; \
         builtin unlinkat --dirfd %h victim; builtin echo rc=$?",
    );
    assert!(stdout(&out).contains("rc=20"), "stdout={:?}", stdout(&out));
}

/// The motivating case: `bind` leaves a socket path; `unlinkat` clears it so a
/// re-`bind` succeeds (without the unlink the second bind is `EADDRINUSE` 98).
#[test]
fn socket_path_lifecycle() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin bind sock %>%s; builtin unlinkat sock; \
         builtin bind sock %>%s2; builtin echo rc=$?",
    );
    assert!(
        stdout(&out).contains("rc=0"),
        "stdout={:?} stderr={}",
        stdout(&out),
        stderr(&out)
    );
}

/// Parse errors are exit 1 with a report on stderr; no args is `Help` (exit 0).
#[test]
fn parse_errors() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin unlinkat --bad f");
    assert_eq!(out.status.code(), Some(1));
    assert!(!stderr(&out).is_empty(), "stderr should carry the report");

    let out = run(&dir, "builtin unlinkat --flags NOPE f");
    assert_eq!(out.status.code(), Some(1));
    assert!(!stderr(&out).is_empty(), "stderr should carry the report");

    let out = run(&dir, "builtin unlinkat");
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
    let out = run(&dir, "builtin unlinkat --help");
    assert_eq!(out.status.code(), Some(0));
}

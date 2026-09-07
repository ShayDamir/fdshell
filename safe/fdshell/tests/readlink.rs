#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with a 5-byte `f` file, a `sub` dir with `g`, and symlinks
/// `link` -> `f` and `sub/slink` -> `g`; removed on drop.
struct Scratch(PathBuf);

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Scratch {
    fn new() -> Self {
        // Tests in one binary run on parallel threads (same pid), so the
        // dir must be unique per test, not per process.
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("fdshell-readlink-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f"), b"12345").unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("g"), b"67890").unwrap();
        std::os::unix::fs::symlink("f", dir.join("link")).unwrap();
        std::os::unix::fs::symlink("g", dir.join("sub").join("slink")).unwrap();
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

/// Path form prints the stored target, not the resolved file.
#[test]
fn path_form_prints_target() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin readlink link");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(stdout(&out), "f\n");
}

/// `--dir` resolves the link against the directory fd, not the CWD.
#[test]
fn dir_fd_changes_resolution_base() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDONLY sub %>%d; \
                  builtin readlink slink --dir %d; builtin echo rc=$?";
    let out = run(&dir, script);
    assert!(stdout(&out).contains("g\n"), "stdout={:?}", stdout(&out));
    assert!(stdout(&out).contains("rc=0"), "stderr={}", stderr(&out));
    // `slink` does not exist in the CWD, so the same path without `--dir`
    // exits with ENOENT (2).
    let out = run(&dir, "builtin readlink slink; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
}

/// Fd form reads the link behind the handle: O_PATH keeps it on the link.
#[test]
fn fd_form_reads_the_handle() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags \"O_PATH|O_NOFOLLOW\" link %>%h; \
                  builtin readlink %h; builtin echo rc=$?";
    let out = run(&dir, script);
    assert!(stdout(&out).contains("f\n"), "stdout={:?}", stdout(&out));
    assert!(stdout(&out).contains("rc=0"), "stderr={}", stderr(&out));
}

/// A non-symlink path exits with EINVAL (22); a regular-file fd exits with
/// ENOENT (2) — the kernel's own error for an empty path behind a non-link.
#[test]
fn non_link_errors() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin readlink f; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=22"), "stdout={:?}", stdout(&out));
    let script = "builtin openat2 --flags O_RDWR f %>%h; \
                  builtin readlink %h; builtin echo rc=$?";
    let out = run(&dir, script);
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
}

/// A missing path exits with ENOENT (2) and an unset fd var fails with rc 1.
#[test]
fn errors() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin readlink missing; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
    let out = run(&dir, "builtin readlink %nope");
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("fd variable"),
        "stderr={}",
        stderr(&out)
    );
    // A non-directory fd as the resolution base exits with ENOTDIR (20).
    let script = "builtin openat2 --flags O_RDWR f %>%h; \
                  builtin readlink link --dir %h; builtin echo rc=$?";
    let out = run(&dir, script);
    assert!(stdout(&out).contains("rc=20"), "stdout={:?}", stdout(&out));
}

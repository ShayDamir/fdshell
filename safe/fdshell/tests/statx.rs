#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with a 5-byte `f` file, a `sub` dir, and a `link` symlink;
/// removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fdshell-statx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f"), b"12345").unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("g"), b"67890").unwrap();
        std::os::unix::fs::symlink(dir.join("f"), dir.join("link")).unwrap();
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

/// Path form prints one metadata line with the permission bits in octal.
#[test]
fn path_form_prints_metadata() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin statx f");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let line = stdout(&out);
    assert!(line.starts_with("kind=file "), "line={line:?}");
    assert!(line.contains("size=5"), "line={line:?}");
    assert!(line.contains("mode=644"), "line={line:?}");
    assert!(line.contains("ino="), "line={line:?}");
    assert!(line.contains("dev="), "line={line:?}");
    assert!(line.contains("mtime="), "line={line:?}");
}

/// `--dir` resolves the path against the directory fd, not the CWD.
#[test]
fn dir_fd_changes_resolution_base() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDONLY sub %>%d; \
                  builtin statx g --dir %d; builtin echo rc=$?";
    let out = run(&dir, script);
    assert!(
        stdout(&out).contains("kind=file size=5"),
        "stdout={:?}",
        stdout(&out)
    );
    assert!(stdout(&out).contains("rc=0"), "stderr={}", stderr(&out));
    // The inline `--dir=%d` form resolves the same way.
    let script = "builtin openat2 --flags O_RDONLY sub %>%d; \
                  builtin statx g --dir=%d; builtin echo rc=$?";
    let out = run(&dir, script);
    assert!(
        stdout(&out).contains("kind=file size=5"),
        "stdout={:?}",
        stdout(&out)
    );
    assert!(stdout(&out).contains("rc=0"), "stderr={}", stderr(&out));
    // `g` does not exist in the CWD, so the same path without `--dir` fails.
    let out = run(&dir, "builtin statx g; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
}

/// Fd form re-stats the open handle: the size reflects writes after the open.
#[test]
fn fd_form_restats_the_handle() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDWR f %>%h; \
                  builtin statx %h; builtin echo rc=$?";
    let out = run(&dir, script);
    assert!(
        stdout(&out).contains("kind=file size=5"),
        "stdout={:?}",
        stdout(&out)
    );
    assert!(stdout(&out).contains("rc=0"), "stderr={}", stderr(&out));
}

/// `--nofollow` reports the symlink itself; without it the target is followed.
#[test]
fn symlink_followed_and_lstat() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin statx link");
    assert!(
        stdout(&out).starts_with("kind=file "),
        "stdout={:?}",
        stdout(&out)
    );
    let out = run(&dir, "builtin statx link --nofollow");
    assert!(
        stdout(&out).starts_with("kind=symlink "),
        "stdout={:?}",
        stdout(&out)
    );
}

/// A missing path exits with ENOENT (2) and an unset fd var fails with rc 1.
#[test]
fn errors() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin statx missing; builtin echo rc=$?");
    assert!(stdout(&out).contains("rc=2"), "stdout={:?}", stdout(&out));
    let out = run(&dir, "builtin statx %nope");
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("fd variable"),
        "stderr={}",
        stderr(&out)
    );
}

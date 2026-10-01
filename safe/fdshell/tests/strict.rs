#![allow(clippy::unwrap_used)]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir holding a readable `f`; each test gets its own dir (pid +
/// counter, since tests in one binary share the pid); removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fdshell-strict-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f"), b"hello").unwrap();
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

/// Strict mode rejects an absolute path, naming the strict/absolute violation.
#[test]
fn strict_bans_absolute_path() {
    let dir = Scratch::new();
    let f = dir.join("f");
    let abs = f.to_str().unwrap();
    // A `--dirfd` is present, so the absolute-path ban (not the dirfd ban)
    // is what fires.
    let out = run(
        &dir,
        &format!("shopt -s strict; builtin openat2 --dirfd %CWD --flags O_RDONLY {abs} %>%f"),
    );
    assert!(
        !out.status.success(),
        "stdout={:?} stderr={:?}",
        stdout(&out),
        stderr(&out)
    );
    let err = stderr(&out);
    assert!(err.contains("strict"), "stderr={err:?}");
    assert!(err.contains("absolute"), "stderr={err:?}");
}

/// Strict mode accepts an explicit `--dirfd` + relative path.
#[test]
fn strict_allows_explicit_dirfd_relative() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "shopt -s strict; builtin openat2 --dirfd %CWD --flags O_RDONLY f %>%fd",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
}

/// Strict mode bans an omitted `--dirfd` (the `AT_FDCWD` / CWD form).
#[test]
fn strict_bans_missing_dirfd() {
    let dir = Scratch::new();
    let out = run(&dir, "shopt -s strict; builtin mkdirat sub %>%d");
    assert!(
        !out.status.success(),
        "stdout={:?} stderr={:?}",
        stdout(&out),
        stderr(&out)
    );
    let err = stderr(&out);
    assert!(err.contains("strict"), "stderr={err:?}");
    assert!(err.contains("dirfd"), "stderr={err:?}");
}

/// Strict mode accepts `renameat2` with both dirfds pinned and relative paths.
#[test]
fn strict_allows_renameat2_with_dirfds() {
    let dir = Scratch::new();
    std::fs::write(dir.join("a"), b"x").unwrap();
    let out = run(
        &dir,
        "shopt -s strict; builtin renameat2 --olddirfd %CWD --newdirfd %CWD a b",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert!(dir.join("b").exists(), "b should exist after rename");
    assert!(!dir.join("a").exists(), "a should be gone after rename");
}

/// Strict is off by default: an absolute path with no dirfd still works
/// (no regression for existing CWD-based scripts).
#[test]
fn default_allows_absolute_path() {
    let dir = Scratch::new();
    let f = dir.join("f");
    let abs = f.to_str().unwrap();
    let out = run(
        &dir,
        &format!("builtin openat2 --flags O_RDONLY {abs} %>%fd"),
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
}

/// The option list reports `strict off`, then `strict on` after enabling.
#[test]
fn shopt_lists_strict_off_then_on() {
    let dir = Scratch::new();
    let out = run(&dir, "shopt");
    assert!(
        stdout(&out).contains("strict off"),
        "stdout={:?}",
        stdout(&out)
    );
    let out = run(&dir, "shopt -s strict; shopt");
    assert!(
        stdout(&out).contains("strict on"),
        "stdout={:?}",
        stdout(&out)
    );
}

/// `set -o strict` / `set +o strict` toggle the option (query exit codes).
#[test]
fn set_dash_o_toggles_strict() {
    let dir = Scratch::new();
    let out = run(&dir, "set -o strict; shopt -q strict");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let out = run(&dir, "set +o strict; shopt -q strict");
    assert!(!out.status.success(), "stderr={}", stderr(&out));
}

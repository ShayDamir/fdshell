//! E2E: the `utimensat` builtin — set file atime/mtime.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with files `f` and `t`, and a symlink `link` -> `t`; removed
/// on drop.
struct Scratch(PathBuf);

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Scratch {
    fn new() -> Self {
        // Tests in one binary run on parallel threads (same pid), so the
        // dir must be unique per test, not per process.
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "fdshell-utimensat-e2e-{}-{}",
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f"), b"file").unwrap();
        std::fs::write(dir.join("t"), b"target").unwrap();
        std::os::unix::fs::symlink("t", dir.join("link")).unwrap();
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

/// The `mtime=` value from a `statx` one-line output.
fn mtime_of(out: &Output) -> Option<i64> {
    stdout(out)
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("mtime="))
        .and_then(|v| v.parse::<i64>().ok())
}

/// `--mtime EPOCH` sets the mtime; `statx` reports it back.
#[test]
fn sets_mtime() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin utimensat --mtime 1234567890 f; builtin statx f",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(mtime_of(&out), Some(1234567890));
}

/// No specs defaults both to `now`: the mtime lands near the current time.
#[test]
fn no_specs_defaults_to_now() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin utimensat f; builtin statx f");
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let m = mtime_of(&out).unwrap();
    let real = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(
        (m - real).abs() <= 10,
        "mtime {m} not within 10s of now {real}"
    );
}

/// `--flags AT_SYMLINK_NOFOLLOW` updates the link's own mtime, not the target's.
#[test]
fn nofollow_changes_link_not_target() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin utimensat --atime 2222222222 --mtime 2222222222 t; \
         builtin utimensat --flags AT_SYMLINK_NOFOLLOW --mtime 1234567890 link; \
         builtin statx link --nofollow",
    );
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert_eq!(mtime_of(&out), Some(1234567890), "link mtime must be set");
    // The target's mtime is untouched.
    let out = run(&dir, "builtin statx t");
    assert_eq!(
        mtime_of(&out),
        Some(2222222222),
        "target mtime must be untouched"
    );
}

/// Parse errors are exit 1 with a report on stderr; no args and `--help` exit 0.
#[test]
fn parse_errors() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin utimensat --bad f");
    assert_eq!(out.status.code(), Some(1));
    assert!(!stderr(&out).is_empty(), "stderr should carry the report");

    let out = run(&dir, "builtin utimensat --mtime banana f");
    assert_eq!(out.status.code(), Some(1));
    assert!(!stderr(&out).is_empty(), "stderr should carry the report");

    let out = run(&dir, "builtin utimensat");
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
    let out = run(&dir, "builtin utimensat --help");
    assert_eq!(out.status.code(), Some(0));
}

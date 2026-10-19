#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it
/// runs on a failing test's unwind, which is exactly the test that used to
/// leak.
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A pid + counter scratch dir named after `tag`; returns it plus the guard
/// — bind both.
fn scratch_dir(tag: &str) -> (std::path::PathBuf, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("builtin_first_{tag}_{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    let scratch = Scratch(dir.clone());
    (dir, scratch)
}

fn make_shadow(tag: &str) -> (std::path::PathBuf, Scratch) {
    let (dir, scratch) = scratch_dir(tag);
    let shadow = dir.join("openat2");
    std::fs::write(&shadow, "#!/bin/sh\necho EXTERNAL\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shadow, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (shadow, scratch)
}

fn run_with_path(path: &str, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .env("PATH", path)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn path_with(dir: &str) -> String {
    format!("{dir}:{}", std::env::var("PATH").unwrap_or_default())
}

#[test]
fn without_option_bare_name_reaches_path() {
    let (shadow, _scratch) = make_shadow("off");
    let dir = shadow.parent().unwrap().to_str().unwrap();
    let (out, _err, _code) = run_with_path(&path_with(dir), "openat2");
    assert_eq!(out, "EXTERNAL\n");
}

#[test]
fn builtin_first_resolves_bare_name_as_builtin() {
    let (shadow, _scratch) = make_shadow("on");
    let dir = shadow.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(
        &path_with(dir),
        "set -o builtin_first; openat2 missing_file_xyz %>%z",
    );
    assert_ne!(code, 0, "stderr={err:?}");
    assert!(!out.contains("EXTERNAL"), "stdout={out:?}");
}

#[test]
fn builtin_first_still_allows_explicit_path() {
    let (shadow, _scratch) = make_shadow("path");
    let dir = shadow.parent().unwrap().to_str().unwrap();
    let (out, err, code) = run_with_path(
        &path_with(dir),
        &format!("set -o builtin_first; {}", shadow.display()),
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "EXTERNAL\n");
}

#[test]
fn builtin_keyword_still_works_with_option_off() {
    let (out, _err, code) = run_with_path("/usr/bin:/bin", "builtin echo kw");
    assert_eq!(code, 0);
    assert_eq!(out, "kw\n");
}

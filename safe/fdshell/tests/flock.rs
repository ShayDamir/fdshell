#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with a regular `lock` file; removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fdshell-flock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lock"), b"").unwrap();
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

/// An exclusive lock is held by the shell after the builtin returns; a second
/// open of the same file fails `--nowait` with EWOULDBLOCK (exit 11).
#[test]
fn lock_persists_and_nowait_conflicts() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDWR lock %>%a; \
                  builtin openat2 --flags O_RDWR lock %>%b; \
                  builtin flock %a; builtin echo locked=$?; \
                  builtin flock %b --nowait; builtin echo blocked=$?; \
                  builtin flock %b --shared --nowait; builtin echo shared=$?; \
                  builtin flock %a --unlock; builtin echo unlocked=$?; \
                  builtin flock %b --nowait; builtin echo reacquired=$?";
    let out = run(&dir, script);
    assert_eq!(
        stdout(&out),
        "locked=0\nblocked=11\nshared=11\nunlocked=0\nreacquired=0\n",
        "stderr={}",
        stderr(&out)
    );
}

/// A shared lock does not conflict with itself but blocks an exclusive one.
#[test]
fn shared_locks_coexist() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDWR lock %>%a; \
                  builtin openat2 --flags O_RDWR lock %>%b; \
                  builtin flock %a --shared; builtin echo a=$?; \
                  builtin flock %b --shared --nowait; builtin echo b=$?; \
                  builtin flock %b --nowait; builtin echo excl=$?";
    let out = run(&dir, script);
    assert_eq!(
        stdout(&out),
        "a=0\nb=0\nexcl=11\n",
        "stderr={}",
        stderr(&out)
    );
}

/// An unset fd variable and a bad flag both fail with a message and rc 1.
#[test]
fn bad_arguments_error() {
    let dir = Scratch::new();
    for (script, what) in [
        ("builtin flock %nope", "fd variable"),
        (
            "builtin openat2 --flags O_RDWR lock %>%a; builtin flock %a --bogus",
            "flag",
        ),
    ] {
        let out = run(&dir, script);
        assert_eq!(out.status.code(), Some(1), "script={script}");
        assert!(
            stderr(&out).contains(what),
            "script={script} stderr={}",
            stderr(&out)
        );
    }
}

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with a `sub` subdir holding `alpha`, `beta`, `gamma`; removed
/// on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fdshell-ls-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for name in ["alpha", "beta", "gamma"] {
            std::fs::write(dir.join("sub").join(name), name.as_bytes()).unwrap();
        }
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

/// Path form opens the directory itself and prints each entry name on a line.
#[test]
fn path_form_lists_entries() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin ls sub");
    assert_eq!(out.status.code(), Some(0), "stderr={}", stderr(&out));
    let s = stdout(&out);
    for name in [".", "..", "alpha", "beta", "gamma"] {
        assert!(s.lines().any(|line| line == name), "output={s:?}");
    }
}

/// Fd form lists the entries behind an open directory fd variable.
#[test]
fn fd_form_lists_entries() {
    let dir = Scratch::new();
    let out = run(
        &dir,
        "builtin openat2 --flags O_RDONLY sub %>%d; builtin ls %d",
    );
    assert_eq!(out.status.code(), Some(0), "stderr={}", stderr(&out));
    let s = stdout(&out);
    assert!(s.lines().any(|line| line == "alpha"), "output={s:?}");
    assert!(s.lines().any(|line| line == "gamma"), "output={s:?}");
}

/// An unset fd variable fails with a message and rc 1.
#[test]
fn unset_fd_var_errors() {
    let dir = Scratch::new();
    let out = run(&dir, "builtin ls %nope");
    assert_eq!(out.status.code(), Some(1), "script=ls %nope");
    assert!(
        stderr(&out).contains("no fd variable with that name is set"),
        "stderr={}",
        stderr(&out)
    );
}

/// A regular file passed to path form is not a directory: O_DIRECTORY fails the
/// open and the builtin returns ENOTDIR (20), matching the syscall-error rc.
#[test]
fn non_directory_errors() {
    let dir = Scratch::new();
    std::fs::write(dir.join("file"), b"x").unwrap();
    let out = run(&dir, "builtin ls file");
    assert_eq!(out.status.code(), Some(20), "script=ls file");
}

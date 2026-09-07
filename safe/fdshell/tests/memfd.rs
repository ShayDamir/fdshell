#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// An empty scratch dir, removed on drop. `memfd` needs no files on disk.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fdshell-memfd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl std::ops::Deref for Scratch {
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

/// No flags create an anonymous memfd. Capturing it into `%m` and re-statting
/// it via `statx %m` proves the handle is valid (rc 0) and empty (size 0).
#[test]
fn anonymous_memfd_lists_as_empty_file() {
    let dir = Scratch::new();
    let script = "builtin memfd %>%m; builtin statx %m";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("kind=file size=0 "),
        "script={script} stdout={}",
        stdout(&out)
    );
}

/// `--name SECRET` creates a named memfd; capturing and re-statting it still
/// yields a valid regular file.
#[test]
fn named_memfd_lists_as_regular_file() {
    let dir = Scratch::new();
    let script = "builtin memfd --name secret %>%m; builtin statx %m";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("kind=file "),
        "script={script} stdout={}",
        stdout(&out)
    );
}

/// `--size 100` ftruncates the memfd to 100 bytes; `statx` reports that size.
#[test]
fn sized_memfd_reports_requested_size() {
    let dir = Scratch::new();
    let script = "builtin memfd --size 100 %>%m; builtin statx %m";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("size=100 "),
        "script={script} stdout={}",
        stdout(&out)
    );
}

/// `--help` exits 0 without creating or capturing anything.
#[test]
fn help_exits_zero() {
    let dir = Scratch::new();
    let script = "builtin memfd --help";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
}

/// A name containing `/` is rejected with a message and rc 1.
#[test]
fn name_with_slash_is_rejected() {
    let dir = Scratch::new();
    let script = "builtin memfd --name /x %>%m";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("name"),
        "script={script} stderr={}",
        stderr(&out)
    );
}

/// A non-numeric size is rejected with a message and rc 1.
#[test]
fn non_numeric_size_is_rejected() {
    let dir = Scratch::new();
    let script = "builtin memfd --size abc %>%m";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("size"),
        "script={script} stderr={}",
        stderr(&out)
    );
}

/// An unknown flag is rejected with a message and rc 1.
#[test]
fn unknown_flag_is_rejected() {
    let dir = Scratch::new();
    let script = "builtin memfd --bogus %>%m";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("flag"),
        "script={script} stderr={}",
        stderr(&out)
    );
}

/// An unknown seal name is rejected with a message and rc 1.
#[test]
fn unknown_seal_is_rejected() {
    let dir = Scratch::new();
    let script = "builtin memfd --seal BAD %>%m";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(1),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("seal"),
        "script={script} stderr={}",
        stderr(&out)
    );
}

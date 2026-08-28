#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A scratch dir with `a.txt` ("hello") and a subdir `d`; removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("fdshell-fdops-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("d")).unwrap();
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
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

/// `lseek %f OFFSET` prints the resulting offset; WHENCE 0/1/2 select
/// SET/CUR/END (default SET).
#[test]
fn lseek_prints_new_offsets() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDWR a.txt %>%f; \
                  builtin lseek %f 2; builtin lseek %f 0 1; \
                  builtin lseek %f 0 2; builtin lseek %f -3 2";
    let out = run(&dir, script);
    assert_eq!(stdout(&out), "2\n2\n5\n2\n", "stderr={}", stderr(&out));
    assert!(out.status.success(), "stderr={}", stderr(&out));
}

/// `ftruncate %f LENGTH` shrinks and extends the file; `wc -c` observes it.
#[test]
fn ftruncate_resizes_to_length() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDWR a.txt %>%f; \
                  builtin ftruncate %f 2; wc -c a.txt; \
                  builtin ftruncate %f 8; wc -c a.txt";
    let out = run(&dir, script);
    assert!(out.status.success(), "stderr={}", stderr(&out));
    let out = stdout(&out);
    assert!(out.contains("2 a.txt"), "stdout={out}");
    assert!(out.contains("8 a.txt"), "stdout={out}");
}

/// `ftruncate %f` without a length truncates at the current offset.
#[test]
fn ftruncate_defaults_to_current_offset() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDWR a.txt %>%f; \
                  builtin lseek %f 4; builtin ftruncate %f; wc -c a.txt";
    let out = run(&dir, script);
    assert!(out.status.success(), "stderr={}", stderr(&out));
    assert!(
        stdout(&out).contains("4 a.txt"),
        "stdout={} stderr={}",
        stdout(&out),
        stderr(&out)
    );
}

/// `fsync %f` succeeds on a real file.
#[test]
fn fsync_succeeds() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDWR a.txt %>%f; builtin fsync %f; builtin echo rc=$?";
    let out = run(&dir, script);
    assert_eq!(stdout(&out), "rc=0\n", "stderr={}", stderr(&out));
}

/// An unset fd variable fails cleanly for all three builtins.
#[test]
fn unset_fd_var_errors() {
    let dir = Scratch::new();
    for script in [
        "builtin lseek %nope 1",
        "builtin ftruncate %nope 3",
        "builtin fsync %nope",
    ] {
        let out = run(&dir, script);
        assert_eq!(out.status.code(), Some(1), "script={script}");
        assert!(
            stderr(&out).contains("fd variable"),
            "script={script} stderr={}",
            stderr(&out)
        );
    }
}

/// Bad or missing numeric arguments fail cleanly with a message.
#[test]
fn bad_arguments_error() {
    let dir = Scratch::new();
    for (op, what) in [
        ("builtin lseek %f", "offset"),
        ("builtin lseek %f 1 9", "whence"),
        ("builtin ftruncate %f nope", "length"),
        ("builtin ftruncate %f -1", "length"),
        ("builtin fsync %f yet-another", "arg"),
    ] {
        let script = format!("builtin openat2 --flags O_RDWR a.txt %>%f; {op}");
        let out = run(&dir, &script);
        assert_eq!(out.status.code(), Some(1), "script={script}");
        assert!(
            stderr(&out).contains(what),
            "script={script} stderr={}",
            stderr(&out)
        );
    }
}

/// A syscall error surfaces as the errno exit code with no stderr.
#[test]
fn ftruncate_directory_is_einval() {
    let dir = Scratch::new();
    let script = "builtin openat2 --flags O_RDONLY --resolve RESOLVE_BENEATH d %>%d; \
                  builtin ftruncate %d 0; builtin echo rc=$?";
    let out = run(&dir, script);
    assert_eq!(stdout(&out), "rc=22\n", "stderr={}", stderr(&out));
    assert_eq!(stderr(&out), "", "errno errors must be silent");
}

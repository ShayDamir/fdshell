//! E2E bash-parity tests for the `read` builtin's flag families: default
//! backslash processing, `-r`, `-d delim` (including `-d ''`), `-t` timeout,
//! and the EOF status rule (EOF before the delimiter → 1, even with data).
//!
//! `read` rejects direct redirects, so stdin is fed through `exec < file`;
//! a fifo (O_RDWR, no peer) is the deterministic "open, empty, no EOF"
//! source for the `-t 0` timeout case.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Per-test scratch dir, removed on drop. Named with pid + an atomic counter:
/// tests in one binary share the process and run on parallel threads, so a
/// pid-only name would collide (LESSONS).
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fdshell-read-{}-{}", std::process::id(), c));
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

/// Default mode processes backslashes: `a\nb` (a, backslash, n, b) → `anb`.
#[test]
fn default_backslash_processing() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\nb\\n" > in; exec < in; read x; printf "[%s]\n" "$x""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[anb]\n", "script={script}");
}

/// `-r` keeps the backslash literal: `a\nb` stays `a\nb`.
#[test]
fn r_keeps_backslash_literal() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\nb\\n" > in; exec < in; read -r x; printf "[%s]\n" "$x""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a\\nb]\n", "script={script}");
}

/// A backslash + newline is a continuation: both bytes are dropped.
#[test]
fn continuation_line_dropped() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\\\nb\\n" > in; exec < in; read x; printf "[%s]\n" "$x""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[ab]\n", "script={script}");
}

/// A trailing backslash at EOF is dropped (bash behavior), status 1.
#[test]
fn trailing_backslash_at_eof_dropped() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\" > in; exec < in; read x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a] rc=1\n", "script={script}");
}

/// `-r` keeps a trailing backslash at EOF literal, status 1.
#[test]
fn trailing_backslash_at_eof_raw_kept() {
    let dir = Scratch::new();
    let script = r#"printf "a\\\\" > in; exec < in; read -r x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a\\] rc=1\n", "script={script}");
}

/// `-d :` stops at the colon: `a:b:c` → `a`, status 0.
#[test]
fn colon_delimiter() {
    let dir = Scratch::new();
    let script = r#"printf "a:b:c" > in; exec < in; read -d : x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a] rc=0\n", "script={script}");
}

/// `-d ':|'` stops at the first byte of the set: `ab|cd` → `ab`, status 0.
#[test]
fn multi_char_delimiter_set() {
    let dir = Scratch::new();
    let script =
        r#"printf "ab|cd" > in; exec < in; read -d ":|" x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[ab] rc=0\n", "script={script}");
}

/// `-d ''` reads the whole stream; EOF is still a failure (status 1), data kept.
#[test]
fn empty_delim_reads_to_eof() {
    let dir = Scratch::new();
    let script = r#"printf "a:b:c" > in; exec < in; read -d "" x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[a:b:c] rc=1\n", "script={script}");
}

/// EOF before the newline delimiter → status 1, even with partial data.
#[test]
fn eof_without_newline_status_1() {
    let dir = Scratch::new();
    let script = r#"printf "abc" > in; exec < in; read x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[abc] rc=1\n", "script={script}");
}

/// A newline-terminated line → status 0.
#[test]
fn newline_terminated_status_0() {
    let dir = Scratch::new();
    let script = r#"printf "abc\\n" > in; exec < in; read x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[abc] rc=0\n", "script={script}");
}

/// `-t 0` with data immediately available reads it, status 0.
#[test]
fn timeout_zero_data_ready() {
    let dir = Scratch::new();
    let script = r#"printf "hi\\n" > in; exec < in; read -t 0 x; printf "[%s] rc=%s\n" "$x" "$?""#;
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "[hi] rc=0\n", "script={script}");
}

/// `-t 0` on an open, empty fifo (O_RDWR: no data, no EOF) times out:
/// status 1, the variable is unset.
#[test]
fn timeout_zero_no_data_fifo() {
    let dir = Scratch::new();
    let script = "builtin mkfifoat --mode 0600 ch %>%ch; \
                  builtin openat2 --flags O_RDWR ch %>%p; \
                  read -t 0 L -u %p; \
                  echo $?:$L";
    let out = run(&dir, script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "script={script} stderr={}",
        stderr(&out)
    );
    // status 1 and the variable left unset: an unset `$L` expands to empty, so
    // the word collapses to `1:`.
    assert_eq!(stdout(&out), "1:\n", "script={script}");
}

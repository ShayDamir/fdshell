#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN).args(["-c", script]).output().unwrap()
}

/// `wait $!` reaps the backgrounded child and returns its exit code.
#[test]
fn wait_by_pid_returns_child_exit_code() {
    let out = run("builtin false &>&j; wait $!; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=1"),
        "expected exit=1, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait $!` on a successful child returns 0.
#[test]
fn wait_by_pid_success() {
    let out = run("builtin true &>&j; wait $!; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait` with no args reaps every background task; the last status wins.
#[test]
fn wait_no_args_reaps_all() {
    let out = run("builtin true &>&j; builtin true &>&k; wait; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait 999999` (a pid with no matching task) is a clean error (exit 1).
#[test]
fn wait_unknown_pid_errors() {
    let out = run("wait 999999");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("not found"), "stderr={err}");
}

/// `wait abc` (a non-numeric arg) is a clean error (exit 1).
#[test]
fn wait_non_numeric_bad_pid() {
    let out = run("wait abc");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("pid"), "stderr={err}");
}

/// `builtin wait` is accepted (the builtin-validation path allows it).
#[test]
fn builtin_wait_accepted() {
    let out = run("builtin true &>&j; builtin wait $!; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait readable` opens a `wait` block (pattern keyword lookahead), so a
/// bare `wait readable` with no `done` is a parse error, not the builtin.
#[test]
fn wait_pattern_keyword_is_block_not_builtin() {
    let out = run("wait readable");
    assert_ne!(out.status.code(), Some(0));
}

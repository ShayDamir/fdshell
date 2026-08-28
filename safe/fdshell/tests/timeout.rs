#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN).args(["-c", script]).output().unwrap()
}

/// `timeout 1 sleep 5` must time out: the child is killed and the command
/// exits 124 (matching coreutils `timeout`).
#[test]
fn timeout_times_out() {
    let out = run("timeout 1 sleep 5; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=124"),
        "expected exit=124, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `timeout 5 true` must succeed: the child finishes in time and the command
/// exits 0.
#[test]
fn timeout_success() {
    let out = run("timeout 5 true; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// A child that ignores SIGTERM must still be killed: after the grace period
/// the shell force-SIGKILLs it and the command exits 124.
#[test]
fn timeout_force_kills_sigterm_ignoring_child() {
    // `sh` traps (ignores) SIGTERM, then execs `sleep`, which inherits the
    // ignored disposition. The deadline SIGTERM therefore has no effect and
    // only the grace-period SIGKILL can end the child.
    let out = run("timeout 1 sh -c \"trap \\\"\\\" TERM; exec sleep 30\"; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=124"),
        "expected exit=124, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `timeout 5 builtin false` must return the child's exit code (1), not 124.
#[test]
fn timeout_returns_child_exit_code() {
    let out = run("timeout 5 builtin false; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=1"),
        "expected exit=1, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `timeout` with no seconds is a clean error (exit 1).
#[test]
fn timeout_missing_seconds_errors() {
    let out = run("timeout");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("timeout"), "stderr={err}");
}

/// `timeout 5` with no command is a clean error (exit 1).
#[test]
fn timeout_missing_command_errors() {
    let out = run("timeout 5");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("timeout"), "stderr={err}");
}

/// `timeout abc true` with a bad seconds value is a clean error (exit 1).
#[test]
fn timeout_bad_seconds_errors() {
    let out = run("timeout abc true");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("seconds"), "stderr={err}");
}

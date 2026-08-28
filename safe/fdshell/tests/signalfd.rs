#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// `signalfd %s USR1` creates an fd var that becomes readable when SIGUSR1 is
/// delivered to the shell. The script prints `ready` after creating the
/// signalfd, then waits for it to become readable.
#[test]
fn signalfd_catches_delivered_signal() {
    let script = "signalfd %s USR1; builtin echo ready; \
                  wait readable %s) builtin echo caught ;; done; unset %s";
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    // Wait for the signalfd to be created.
    let first = lines.next().unwrap().unwrap();
    assert_eq!(first, "ready", "first line must be the ready marker");

    // Deliver SIGUSR1 to the shell; the signalfd must catch it.
    let status = sys::kill::kill(sys::Pid::from_raw(pid as i32), sys::signalfd::SIGUSR1);
    assert!(status.is_ok(), "failed to send SIGUSR1: {status:?}");

    let second = lines.next().unwrap().unwrap();
    assert_eq!(
        second, "caught",
        "the signalfd must become readable on SIGUSR1"
    );

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr={:?}",
        str::from_utf8(&output.stderr)
    );
}

/// `signalfd` with no signals is a clean error (exit 1).
#[test]
fn signalfd_no_signals_errors() {
    let output = Command::new(BIN)
        .args(["-c", "signalfd %s"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let err = str::from_utf8(&output.stderr).unwrap();
    assert!(err.contains("signalfd"), "stderr={err}");
}

/// `signalfd` with an unknown signal is a clean error (exit 1).
#[test]
fn signalfd_bad_signal_errors() {
    let output = Command::new(BIN)
        .args(["-c", "signalfd %s FOO"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let err = str::from_utf8(&output.stderr).unwrap();
    assert!(err.contains("not a signal"), "stderr={err}");
}

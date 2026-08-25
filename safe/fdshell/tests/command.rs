#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
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

#[test]
fn command_runs_builtin() {
    let (out, err, code) = run("command echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn command_bypasses_function_lookup() {
    let (out, _err, code) = run("echo() { builtin echo shadowed; }; command echo hi; echo hi");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\nshadowed\n");
}

#[test]
fn command_unknown_name_is_not_a_builtin() {
    let (_out, err, code) = run("command definitely_not_a_cmd_xyz");
    assert_ne!(code, 0);
    assert!(err.contains("definitely_not_a_cmd_xyz"), "stderr={err:?}");
}

#[test]
fn command_prefix_rejected_on_intercepts() {
    let (_out, err, code) = run("command cd /tmp");
    assert_ne!(code, 0);
    assert!(err.contains("prefix is not supported"), "stderr={err:?}");
}

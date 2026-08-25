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
fn set_dash_x_traces_commands_on_stderr() {
    let (out, err, code) = run("set -x; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    assert!(err.contains("+ set -x"), "stderr={err:?}");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
}

#[test]
fn set_plus_x_stops_tracing() {
    let (_out, err, code) = run("set -x; set +x; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ set -x"), "stderr={err:?}");
    // bash traces the disabling command itself.
    assert!(err.contains("+ set +x"), "stderr={err:?}");
    assert!(!err.contains("+ echo hi"), "stderr={err:?}");
}

#[test]
fn xtrace_shows_expanded_args() {
    let (_out, err, code) = run("X=1; set -x; builtin echo $X");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo 1"), "stderr={err:?}");
}

#[test]
fn xtrace_covers_intercepts() {
    let (_out, err, code) = run("set -x; shift; builtin echo ok");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ shift"), "stderr={err:?}");
}

#[test]
fn xtrace_covers_pipeline_stages() {
    let (_out, err, code) = run("set -x; builtin echo a | builtin echo b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo a"), "stderr={err:?}");
    assert!(err.contains("+ echo b"), "stderr={err:?}");
}

#[test]
fn xtrace_settable_via_set_dash_o() {
    let (_out, err, code) = run("set -o xtrace; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
    let (_out, err, code) = run("shopt -s xtrace; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
}

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
fn bare_set_lists_positionals_and_variables() {
    let (out, err, code) = run("FOO=bar; set -- one two; set");
    assert_eq!(code, 0, "stderr={err:?}");
    for line in ["one", "two", "FOO=bar", "_=two"] {
        assert!(
            out.lines().any(|l| l == line),
            "missing {line:?} in {out:?}"
        );
    }
    // Positionals are listed raw, without `NAME=`.
    assert!(!out.contains("one="));
}

#[test]
fn set_lists_exported_variables() {
    let (out, err, code) = run("export P=1; set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == "P=1"), "stdout={out:?}");
}

#[test]
fn set_dash_f_lists_fd_variables() {
    let (out, err, code) = run("set -F");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == "%CWD"), "stdout={out:?}");
}

#[test]
fn set_dash_f_is_empty_without_fd_vars() {
    let (out, err, code) = run("set -F");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().all(|l| l.starts_with('%')), "stdout={out:?}");
}

#[test]
fn set_lists_variables_in_sorted_order() {
    let (out, err, code) = run("ZED=1; ALPHA=2; export MID=9; set");
    assert_eq!(code, 0, "stderr={err:?}");
    // IFS's default value is " \t\n", so its line is followed by a blank one.
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        ["sh", "ALPHA=2", "IFS= \t", "", "MID=9", "ZED=1", "_=MID=9"]
    );
}

#[test]
fn set_lists_ifs_exactly_once() {
    let (out, err, code) = run("set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.lines().collect::<Vec<_>>(), ["sh", "IFS= \t", ""]);
    // IFS already assigned: still listed exactly once.
    let (out, err, code) = run("IFS=z; set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.lines().collect::<Vec<_>>(), ["sh", "IFS=z", "_="]);
}

#[test]
fn set_lists_exported_variable_not_twice() {
    let (out, err, code) = run("Q=3; export Q; set");
    assert_eq!(code, 0, "stderr={err:?}");
    let qs: Vec<&str> = out.lines().filter(|l| *l == "Q=3").collect();
    assert_eq!(qs, ["Q=3"], "stdout={out:?}");
}

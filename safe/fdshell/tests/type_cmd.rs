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
fn type_reports_builtins_and_keywords() {
    let (out, _err, code) = run("builtin type echo");
    assert_eq!(code, 0, "stdout={out:?}");
    assert!(out.contains("echo is a shell builtin"), "stdout={out:?}");
    let (out, _err, code) = run("builtin type if");
    assert_eq!(code, 0);
    assert!(out.contains("if is a shell keyword"), "stdout={out:?}");
}

#[test]
fn type_is_a_builtin_itself() {
    let (out, err, code) = run("type type");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.contains("type is a shell builtin"), "stdout={out:?}");
}

#[test]
fn type_reports_external_with_path() {
    let (out, _err, code) = run("builtin type sh");
    assert_eq!(code, 0);
    assert!(out.contains("sh is /"), "stdout={out:?}");
}

#[test]
fn type_reports_function_and_alias() {
    let (out, _err, code) = run(r#"f() { true; }; alias ab="echo hi"; type f ab"#);
    assert_eq!(code, 0, "stdout={out:?}");
    assert!(out.contains("f is a shell function"), "stdout={out:?}");
    assert!(out.contains("ab is aliased to 'echo hi'"), "stdout={out:?}");
}

#[test]
fn type_reports_fd_variable() {
    let (out, _err, code) = run("builtin type CWD");
    assert_eq!(code, 0, "stdout={out:?}");
    assert!(out.contains("CWD is an fd variable"), "stdout={out:?}");
}

#[test]
fn type_missing_name_fails() {
    let (out, err, code) = run("builtin type definitely_not_a_cmd_xyz");
    assert_eq!(code, 1, "stdout={out:?} stderr={err:?}");
    assert!(
        err.contains("definitely_not_a_cmd_xyz: not found"),
        "stderr={err:?}"
    );
}

#[test]
fn type_missing_name_mixed_with_found() {
    let (out, _err, code) = run("builtin type echo definitely_not_a_cmd_xyz");
    assert_eq!(code, 1);
    assert!(out.contains("echo is a shell builtin"), "stdout={out:?}");
}

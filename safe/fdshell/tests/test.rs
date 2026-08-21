#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn test_file_exists() {
    let (out, _err, code) = run("if test -f /proc/self/exe; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn bracket_file_is_dir() {
    let (out, _err, code) = run("if [ -d /tmp ]; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn bracket_file_is_not_dir() {
    let (out, _err, code) = run("if [ -f /tmp ]; then printf n; else printf y; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_missing_file_is_false() {
    let (out, _err, code) =
        run("if test -e /nonexistent-fdshell-test; then printf n; else printf y; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_string_equality_with_variables() {
    let (out, _err, code) = run("X=abc; if test \"$X\" = abc; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_string_empty_with_variables() {
    let (out, _err, code) =
        run("X=; if test -z \"$X\"; then printf a; fi; X=hi; if test -n \"$X\"; then printf b; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "ab");
}

#[test]
fn test_integer_comparison() {
    let (out, _err, code) = run("if test 10 -gt 9; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_malformed_expression_exits_2() {
    let (out, err, code) = run("test a b c d; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
    assert!(err.contains("test"), "stderr={err:?}");
}

#[test]
fn bracket_missing_closer_exits_2() {
    let (out, err, code) = run("[ -f /tmp; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
    assert!(err.contains("test"), "stderr={err:?}");
}

#[test]
fn bracket_fd_var_operand() {
    let (out, _err, code) = run("cd /tmp; if [ -d %CWD ]; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn bare_test_is_false() {
    let (out, _err, code) = run("test; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

#[test]
fn bracket_wrong_closer_exits_2() {
    let (out, err, code) = run("[ -f /tmp foo; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
    assert!(err.contains("test"), "stderr={err:?}");
}

#[test]
fn test_in_conditional_list() {
    let (out, _err, code) = run("test 1 -eq 1 && printf a; test 1 -eq 2 || printf b");
    assert_eq!(code, 0);
    assert_eq!(out, "ab");
}

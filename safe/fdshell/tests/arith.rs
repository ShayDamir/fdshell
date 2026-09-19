#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap()
}

/// `$((…))` is evaluated in-process and replaced with its decimal result;
/// operators keep C precedence (`*` before `+`).
#[test]
fn arith_expansion_uses_c_precedence() {
    let output = run("echo $((2+3*4))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "14");
}

/// Assignment through `$((x+1))` on the right-hand side: the value is
/// expanded before the variable is set.
#[test]
fn arith_expansion_feeds_assignment() {
    let output = run("x=1; x=$((x+1)); echo $x");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "2");
}

/// Assignment inside the expression returns the new value and stores it,
/// so a later word on the same line sees the update.
#[test]
fn arith_internal_assignment_returns_and_stores() {
    let output = run("echo $((y=3)) $y");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "3 3");
}

/// The ternary picks the taken branch.
#[test]
fn arith_ternary_branches() {
    let t = run("echo $((1?2:3))");
    assert!(t.status.success());
    assert_eq!(str::from_utf8(&t.stdout).unwrap().trim(), "2");
    let f = run("echo $((0?2:3))");
    assert!(f.status.success());
    assert_eq!(str::from_utf8(&f.stdout).unwrap().trim(), "3");
}

/// An unset variable in arithmetic is 0 (no error, no word-splitting).
#[test]
fn arith_unset_variable_is_zero() {
    let output = run("echo $((unsetv+1))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "1");
}

/// `$((…))` inside a quoted word expands in place, keeping the word whole.
#[test]
fn arith_expansion_inside_quoted_word() {
    let output = run("echo \"a$((1+1))b\"");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "a2b");
}

/// Hex and leading-0 octal literals, like bash.
#[test]
fn arith_hex_and_octal_literals() {
    let output = run("echo $((0xff)) $((010))");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "255 8");
}

/// Compound assignment returns the new value and updates the variable.
#[test]
fn arith_compound_assignment_updates_variable() {
    let output = run("x=7; echo $((x+=3)) $x");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "10 10");
}

/// A whole-word `$((…))` in a `for … in` list expands to its value, and the
/// body runs once per expanded word.
#[test]
fn arith_expansion_in_for_list() {
    let output = run("for i in $((1+2)) $((3*3)); do echo $i; done");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "3\n9");
}

/// Division by zero is a clean error that fails the command.
#[test]
fn arith_division_by_zero_fails_command() {
    let output = run("echo $((1/0))");
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("division or modulo by zero"),
        "stderr={stderr}"
    );
    assert!(!output.status.success());
}

/// A malformed expression is a clean syntax error that fails the command.
#[test]
fn arith_malformed_expression_fails_command() {
    let output = run("echo $((1+))");
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("arithmetic expression has a syntax error"),
        "stderr={stderr}"
    );
    assert!(!output.status.success());
}

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
fn unquoted_dollar_escape_is_literal() {
    // bash: `echo [\$_]` prints `[$_]` — the `$` is not expanded.
    let (out, _err, code) = run("true hello; builtin echo [\\$_]");
    assert_eq!(code, 0);
    assert_eq!(out, "[$_]\n");
}

#[test]
fn quoted_dollar_escape_is_literal() {
    let (out, _err, code) = run("true hello; builtin echo \"[\\$_]\"");
    assert_eq!(code, 0);
    assert_eq!(out, "[$_]\n");
}

#[test]
fn quoted_escape_defers_reference_into_eval() {
    // The `\$` defers `$_` into the eval body; the inner `true x y` must not
    // clobber the outer `$_` (eval_depth gating), so the body sees `hello`.
    let (out, err, code) = run("true hello; eval \"true x y; builtin echo [\\$_]\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[hello]\n");
}

#[test]
fn double_backslash_folds_and_keeps_dollar_live() {
    let (out, _err, code) = run("X=1; builtin echo \"a\\\\$X\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\1\n");
}

#[test]
fn unquoted_double_backslash_folds() {
    let (out, _err, code) = run("builtin echo a\\\\b");
    assert_eq!(code, 0);
    assert_eq!(out, "a\\b\n");
}

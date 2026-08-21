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
fn printf_basic_format() {
    let (out, _err, code) = run(r#"printf "%s=%d" hi 42"#);
    assert_eq!(code, 0);
    assert_eq!(out, "hi=42");
}

#[test]
fn printf_with_explicit_newline() {
    // Unquoted: inside double quotes the tokenizer would eat the backslash.
    let (out, _err, code) = run(r"printf %s\n hello");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn printf_reuses_format() {
    let (out, _err, code) = run(r#"printf "[%s] " a b"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a] [b] ");
}

#[test]
fn printf_without_format_prints_newline() {
    let (out, _err, code) = run("printf");
    assert_eq!(code, 0);
    assert_eq!(out, "\n");
}

#[test]
fn printf_builtin_prefix() {
    let (out, _err, code) = run(r#"builtin printf "%d" 7"#);
    assert_eq!(code, 0);
    assert_eq!(out, "7");
}

#[test]
fn printf_invalid_number_fails() {
    let (_out, err, code) = run(r#"printf "%d" abc"#);
    assert_ne!(code, 0);
    assert!(err.contains("number"), "stderr={err:?}");
}

#[test]
fn printf_in_conditional() {
    let (out, _err, code) = run(r#"if printf "%s" x >/dev/null; then printf ok; fi"#);
    assert_eq!(code, 0);
    assert_eq!(out, "ok");
}

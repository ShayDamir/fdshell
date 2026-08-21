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
fn here_string_feeds_stdin() {
    let (out, _err, code) = run("cat <<<hello");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn here_string_quoted_keeps_spaces() {
    let (out, _err, code) = run("cat <<<\"a b\"");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_expands_variables() {
    let (out, _err, code) = run("X=hello; cat <<<$X");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn here_string_bare_form_takes_next_word() {
    let (out, _err, code) = run("cat <<< word");
    assert_eq!(code, 0);
    assert_eq!(out, "word\n");
}

#[test]
fn here_string_appends_trailing_newline() {
    let (out, _err, code) = run("wc -c <<<x");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
}

#[test]
fn here_string_counts_lines() {
    let (out, _err, code) = run("wc -l <<<x");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

#[test]
fn here_string_leaves_other_args_intact() {
    let (out, _err, code) = run("echo a <<<x b");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn here_string_empty_word_is_one_newline() {
    let (out, _err, code) = run("cat <<<\"\"");
    assert_eq!(code, 0);
    assert_eq!(out, "\n");
}

#[test]
fn here_string_with_builtin_command() {
    // printf does not read stdin, but the redirect must apply without error.
    let (out, _err, code) = run("printf \"%s\" a <<<hi");
    assert_eq!(code, 0);
    assert_eq!(out, "a");
}

#[test]
fn here_string_expansion_failure_is_reported() {
    // Out-of-range positional index makes the word expansion fail.
    let (_out, err, code) = run("cat <<<$99999999999999999999");
    assert_ne!(code, 0);
    assert!(err.contains("here-string"), "stderr={err:?}");
}

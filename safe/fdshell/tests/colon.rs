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
fn colon_is_a_noop_returning_zero() {
    let (out, _err, code) = run(":; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
}

#[test]
fn colon_accepts_and_discards_args() {
    let (out, _err, code) = run(": a b c; echo done");
    assert_eq!(code, 0);
    assert_eq!(out, "done\n");
}

#[test]
fn colon_accepts_expanded_arg() {
    // `:` accepts (and discards) an argument that is a param expansion.
    // (The `:=` assignment side effect persisting to the next command is a
    // separate expansion-engine concern, tracked on its own task.)
    let (out, _err, code) = run(": ${x:-unset}; echo done");
    assert_eq!(code, 0);
    assert_eq!(out, "done\n");
}

#[test]
fn colon_heredoc_idiom() {
    // The heredoc body is consumed and discarded; the delimiter must be on its
    // own line, so the following statement is on the next line.
    let (out, _err, code) = run(": <<EOF\ntext\nEOF\necho after");
    assert_eq!(code, 0);
    assert_eq!(out, "after\n");
}

#[test]
fn colon_sets_last_arg() {
    let (out, _err, code) = run(": hello; echo \"$_\"");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn colon_with_builtin_prefix() {
    let (out, _err, code) = run("builtin :; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
}

#[test]
fn colon_with_command_prefix() {
    let (out, _err, code) = run("command :; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
}

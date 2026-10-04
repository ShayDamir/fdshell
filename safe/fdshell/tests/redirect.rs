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

fn temp_path(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("redirect_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

#[test]
fn space_separated_write_redirect() {
    let path = temp_path("write");
    let (out, err, code) = run(&format!("echo hi > {path}; cat {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    // `echo`'s output goes to the file; only `cat` writes to stdout.
    assert_eq!(out, "hi\n");
}

#[test]
fn space_separated_read_redirect() {
    let path = temp_path("read");
    std::fs::write(&path, b"data\n").unwrap();
    let (out, err, code) = run(&format!("cat < {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "data\n");
}

#[test]
fn space_separated_named_fd_redirect() {
    let path = temp_path("stderr");
    // `2> {path}` (the form under test) rehomes fd 2 onto the file; `echo e
    // >&2` then writes through it.
    let (out, err, code) = run(&format!("exec 2> {path}; echo e >&2"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "e\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_append_redirect() {
    let path = temp_path("append");
    let _ = std::fs::remove_file(&path);
    let (out, err, code) = run(&format!("echo a >> {path}; echo b >> {path}; cat {path}"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\nb\n");
}

#[test]
fn space_separated_redirect_quoted_path() {
    let path = std::env::temp_dir().join(format!("redirect_quoted_{}.txt", std::process::id()));
    let quoted = path.to_str().unwrap();
    let (out, err, code) = run(&format!("echo hi > \"{quoted}\"; cat \"{quoted}\""));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn space_separated_redirect_after_exec() {
    let path = temp_path("exec");
    let (out, err, code) = run(&format!("exec > {path}; echo hi"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "hi\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_redirect_in_pipeline() {
    let path = temp_path("pipeline");
    let (out, err, code) = run(&format!("echo a > {path} | cat"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "a\n");
    assert!(out.is_empty());
}

#[test]
fn space_separated_redirect_with_heredoc() {
    let path = temp_path("heredoc");
    let (_out, err, code) = run(&format!("cat <<EOF > {path}\nbody\nEOF"));
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(body, "body\n");
}

#[test]
fn space_separated_redirect_without_operand_is_error() {
    let (_out, err, code) = run("true >");
    assert_eq!(code, 1);
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
}

#[test]
fn space_separated_redirect_before_semicolon_is_error() {
    let (_out, err, code) = run("echo hi > ;");
    assert_eq!(code, 1);
    assert!(err.contains("invalid redirect"), "stderr={err:?}");
}

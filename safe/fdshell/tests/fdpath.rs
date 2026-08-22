#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str, stdin: &str) -> (String, String, i32) {
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn temp_path(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("fdpath_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

#[test]
fn fd_path_redirect_reads_pipe_stdin() {
    let (out, err, code) = run("cat </dev/fd/0", "hi\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn fd_path_redirect_reads_file_fd() {
    let path = temp_path("file");
    std::fs::write(&path, b"abc\n").unwrap();
    let (out, err, code) = run(&format!("exec 3<{path}; cat </dev/fd/3; exec 3>&-"), "");
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "abc\n");
}

#[test]
fn fd_path_redirect_writes_to_fd() {
    let path = temp_path("write");
    let (out, err, code) = run(
        &format!("exec 3>{path}; echo hi >/dev/fd/3; exec 3>&-; cat {path}"),
        "",
    );
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn proc_self_fd_redirect_reads_pipe_stdin() {
    let (out, err, code) = run("cat </proc/self/fd/0", "yo\n");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "yo\n");
}

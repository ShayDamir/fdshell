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
fn dup_redirect_sends_stderr_to_stdout() {
    let (out, err, code) = run("nonexistent_xyz_abc 2>&1");
    assert_ne!(code, 0);
    assert!(out.contains("not found"), "stdout={out:?}");
    assert!(!err.contains("not found"), "stderr={err:?}");
}

#[test]
fn dup_redirect_stdout_to_fd() {
    let (out, err, code) = run("echo err >&2");
    assert_eq!(code, 0);
    assert!(!out.contains("err"), "stdout={out:?}");
    assert!(err.contains("err"), "stderr={err:?}");
}

#[test]
fn exec_fd_dup_then_error_routes_through_fd() {
    let (out, _err, code) = run("exec 5>&1; nonexistent_xyz_abc 2>&5");
    assert_ne!(code, 0);
    assert!(out.contains("not found"), "stdout={out:?}");
}

#[test]
fn close_redirect_closes_fd_in_shell() {
    let (_out, err, code) = run("exec 3>&1; exec 3>&-; exec 4>&3");
    assert_ne!(code, 0);
    assert!(err.contains("fd 3 is not open"), "stderr={err:?}");
}

fn temp_path(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("fddup_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

#[test]
fn exec_fd_dup_writes_via_fd() {
    let path = temp_path("write");
    let (out, err, code) = run(&format!(
        "exec 3>{path}; echo hi >&3; exec 3>&-; cat {path}"
    ));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn exec_fd_dup_reads_via_fd() {
    let path = temp_path("read");
    std::fs::write(&path, b"hi\n").unwrap();
    let (out, err, code) = run(&format!("exec 3<{path}; cat <&3; exec 3>&-"));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
}

#[test]
fn out_of_range_path_redirect_is_actionable() {
    let path = temp_path("range");
    let (_out, err, code) = run(&format!("true 2147483647>{path}"));
    let _ = std::fs::remove_file(&path);
    assert_ne!(code, 0);
    assert!(err.contains("out of range"), "stderr={err:?}");
}

#[test]
fn dup_of_closed_fd_is_actionable() {
    let (_out, err, code) = run("cat 2>&99");
    assert_ne!(code, 0);
    assert!(err.contains("fd 99 is not open"), "stderr={err:?}");
}

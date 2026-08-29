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

/// A dup onto the first free fd (here 4) must stay open for later use. A
/// source dup landing on the target makes `dup2` a no-op whose drop closes
/// the redirected fd.
#[test]
fn exec_dup_onto_first_free_fd_stays_open() {
    let (out, err, code) = run("exec 4>&1; echo via-4 >&4; exec 4>&-; echo done");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "via-4\ndone\n", "stdout={out:?}");
}

/// A path redirect whose target equals the first free fd must not end up
/// closed; data written through the fd must reach the file.
#[test]
fn exec_path_redirect_onto_first_free_fd_survives() {
    let path = temp_path("first_free");
    let (out, err, code) = run(&format!(
        "exec 5>{path}; echo via-5 >&5; exec 5>&-; cat {path}"
    ));
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "via-5\n", "stdout={out:?}");
}

/// Two path redirects whose targets interleave with the pre-opened fd
/// numbers must both keep their targets: one redirect's dup2 must not close
/// the other's target via a pre-opened descriptor that shared its number.
#[test]
fn exec_multi_redirect_targets_keep_their_files() {
    let a = temp_path("multi_a");
    let b = temp_path("multi_b");
    let (out, err, code) = run(&format!(
        "exec 5>{a} 4>{b}; echo to-fd4 >&4; echo to-fd5 >&5; exec 4>&-; exec 5>&-; cat {b} {a}"
    ));
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "to-fd4\nto-fd5\n", "stdout={out:?}");
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

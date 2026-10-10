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
    let path = std::env::temp_dir().join(format!("scope_{tag}_{}.txt", std::process::id()));
    path.to_str().unwrap().to_string()
}

/// Read the redirect target and remove it.
fn body(path: &str) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let _ = std::fs::remove_file(path);
    text
}

fn write_file(path: &str, text: &str) {
    std::fs::write(path, text).unwrap();
}

/// POSIX #2.4: a redirection belongs to the simple command, and a function call
/// is a simple command. `f`'s stdout goes to the file, and the shell's stdout is
/// back for the next command.
#[test]
fn function_write_redirect_applies_and_is_scoped() {
    let a = temp_path("fn_write_a");
    let b = temp_path("fn_write_b");
    let (out, err, code) = run(&format!(
        "f(){{ echo in-f; }}; f > {a}; echo after > {b}; echo A=[$(cat {a})] B=[$(cat {b})]"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "A=[in-f] B=[after]\n", "stdout={out:?}");
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
}

/// `f < in` feeds the function body's stdin; the shell's stdin is untouched.
#[test]
fn function_stdin_redirect_applies() {
    let in_path = temp_path("fn_read_in");
    write_file(&in_path, "hello\n");
    let (out, err, code) = run(&format!(
        "f(){{ read v; echo \"[in-f $v]\"; }}; f < {in_path}; echo after"
    ));
    let _ = std::fs::remove_file(&in_path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[in-f hello]\nafter\n", "stdout={out:?}");
}

/// `f <<EOF` puts the here-doc body on the function body's stdin.
#[test]
fn function_heredoc_applies() {
    let (out, err, code) = run("f(){ cat; echo body; }; f <<EOF\nX\nEOF");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "X\nbody\n", "stdout={out:?}");
}

/// Two redirections on one call: the last to fd 1 wins, and the earlier target
/// was still opened/truncated.
#[test]
fn function_redirect_is_last_wins() {
    let a = temp_path("fn_last_a");
    let b = temp_path("fn_last_b");
    let (out, err, code) = run(&format!("f(){{ echo hi; }}; f > {a} > {b}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "");
    assert_eq!(body(&b), "hi\n");
}

/// A nested call inherits the outer scope, and restore of the outer scope puts
/// the shell's fd 1 back so the next command writes to stdout.
#[test]
fn nested_function_redirect_restores_the_outer_first() {
    let a = temp_path("nested_a");
    let b = temp_path("nested_b");
    let (out, err, code) = run(&format!(
        "f(){{ g(){{ echo g; }}; g; echo f; }}; f > {a}; echo after > {b}"
    ));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "g\nf\n");
    assert_eq!(body(&b), "after\n");
}

/// `cd` is in-process: its redirect is applied by the scope (the file is created
/// in the pre-`cd` directory), and the `cd` itself persists.
#[test]
fn cd_redirect_applies_and_is_scoped() {
    let a = temp_path("cd_a");
    let (out, err, code) = run(&format!("cd /tmp > {a}; pwd; echo after"));
    let _ = std::fs::remove_file(&a);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "/tmp\nafter\n", "stdout={out:?}");
}

/// `eval` runs in the shell, so its redirect reaches the evaluated command.
#[test]
fn eval_redirect_applies() {
    let a = temp_path("eval_a");
    let (out, err, code) = run(&format!("eval echo hi > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "hi\n");
}

/// `source` runs the file in the shell, so the redirect applies to it.
#[test]
fn source_redirect_applies() {
    let src = temp_path("eval_src");
    let a = temp_path("source_a");
    write_file(&src, "echo sourced\n");
    let (out, err, code) = run(&format!("source {src} > {a}"));
    let _ = std::fs::remove_file(&src);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "sourced\n");
}

/// `: ` applies each redirect form (the task #152 pins): a write target is
/// created empty, a read target is consumed and discarded, an error redirect
/// opens the file with nothing written to it.
#[test]
fn colon_applies_redirect() {
    let a = temp_path("colon_write");
    let in_path = temp_path("colon_read");
    let e = temp_path("colon_err");
    write_file(&in_path, "hello\n");
    let (out, err, code) = run(&format!(": > {a}; : < {in_path}; : 2> {e}; echo done"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "done\n", "stdout={out:?}");
    assert_eq!(body(&a), "");
    assert_eq!(body(&e), "");
    assert_eq!(
        body(&in_path),
        "hello\n",
        "a read redirect does not consume the file"
    );
}

/// `shift` is in-process, so its redirect is applied by the scope.
#[test]
fn shift_applies_redirect() {
    let a = temp_path("shift_a");
    let (out, err, code) = run(&format!("shift > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "");
}

/// `local` inside a function: the redirect applies for the command, `local`
/// itself has no output, and stdout is back for the next command.
#[test]
fn local_applies_redirect_inside_a_function() {
    let a = temp_path("local_a");
    let (out, err, code) = run(&format!("f(){{ local x=1 > {a}; echo after; }}; f"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "after\n", "stdout={out:?}");
    assert_eq!(body(&a), "");
}

/// `times` prints through the scope's redirected fd 1; its exact format is
/// fdshell's own, so only the fact that output reached the file is pinned.
#[test]
fn times_redirect_writes_to_the_file() {
    let a = temp_path("times_a");
    let (out, err, code) = run(&format!("times > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert!(!body(&a).is_empty(), "times output must reach the file");
}

/// A forked external under `timeout` inherits the redirection of the command.
#[test]
fn timeout_redirect_reaches_the_child() {
    let a = temp_path("timeout_a");
    let (out, err, code) = run(&format!("timeout 1 echo hi > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "hi\n");
}

/// `exec > file` has no command to run, so the redirect stays: the shell's fd 1
/// is the file for the rest of the script (a scoped restore does not undo it).
#[test]
fn exec_redirect_stays_permanent() {
    let a = temp_path("exec_permanent");
    let (out, err, code) = run(&format!("exec > {a}; echo after"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "after\n");
}

/// `exec > file` inside a function body is scoped to the call: after the call
/// the shell's fd 1 is back, so the next command's redirect writes to its file.
#[test]
fn exec_inside_function_restores_at_call_end() {
    let a = temp_path("exec_in_fn_a");
    let b = temp_path("exec_in_fn_b");
    let (out, err, code) = run(&format!("f(){{ exec > {a}; }}; f; echo after > {b}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "");
    assert_eq!(body(&b), "after\n");
}

/// A `N>&-` target is closed for the duration of the call and reopened at the
/// restore: fd 3 is closed inside `f` and usable again after it.
#[test]
fn closed_fd_redirect_is_restored_after_the_call() {
    let in_path = temp_path("closed_in");
    let a = temp_path("closed_a");
    write_file(&in_path, "hello\n");
    let (out, err, code) = run(&format!(
        "exec 3< {in_path}; f(){{ echo x; }}; f > {a} 3>&-; cat <&3; echo after"
    ));
    let _ = std::fs::remove_file(&in_path);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hello\nafter\n", "stdout={out:?}");
    assert_eq!(body(&a), "x\n");
}

/// A failing redirection open fails the command before its handler runs: the
/// `cd` does not happen. Bash reports rc 1 for the command and continues;
/// fdshell stops the `-c` script at rc 1, so `pwd` never prints.
#[test]
fn failing_redirect_open_fails_the_command_before_the_handler() {
    let (out, err, code) = run("cd /tmp > /nope-dir-x/f; pwd");
    assert_eq!(code, 1, "stderr={err:?}");
    assert_eq!(out, "");
}

/// The same for a function call: the open failure fails the call, so the body
/// never runs and the script stops.
#[test]
fn failing_redirect_open_skips_the_function_body() {
    let (out, err, code) = run("f(){ echo x; }; f > /nope-dir-x/f; echo after");
    assert_eq!(code, 1, "stderr={err:?}");
    assert_eq!(out, "");
}

/// A handler that returns a non-zero status is a success path, so the scope
/// restores: fd 1 is back for the next command, and the `return` status
/// survives the call.
#[test]
fn scoped_restore_survives_a_nonzero_handler_status() {
    let a = temp_path("nonzero_a");
    let (out, err, code) = run(&format!(
        "f(){{ return 3; }}; f > {a}; echo after; f > {a}; true"
    ));
    let _ = std::fs::remove_file(&a);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "after\n", "stdout={out:?}");
}
#[test]
fn capture_on_in_process_builtin_stays_rejected() {
    let a = temp_path("capture_reject");
    let (out, err, code) = run(&format!("read %>%x > {a}"));
    let _ = std::fs::remove_file(&a);
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("captures are not supported"), "stderr={err:?}");
}

/// A function call forks, so its capture and its redirect both work.
#[test]
fn capture_on_function_call_is_accepted() {
    let a = temp_path("capture_call");
    let (out, err, code) = run(&format!("f(){{ echo x; }}; f %>%x > {a}"));
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "");
    assert_eq!(body(&a), "x\n");
}

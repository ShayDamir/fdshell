#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;
use sys::errno::ENOSYS;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn tmpdir() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("fdshell_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn run_fdshell(input: &str, dir: &std::path::Path) -> std::process::Output {
    let mut child = Command::new(BIN)
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn run_c(cmd: &str, dir: &std::path::Path) -> std::process::Output {
    Command::new(BIN)
        .args(["-c", cmd])
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap()
}

fn assert_ok(output: &std::process::Output, msg: &str) {
    assert!(
        output.status.success(),
        "{}: exit={:?} stderr={}",
        msg,
        output.status.code(),
        str::from_utf8(&output.stderr).unwrap()
    );
}

#[test]
fn c_echo() {
    let dir = tmpdir();
    let output = run_c("echo hello", &dir);
    assert_ok(&output, "c_echo");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "hello");
}

#[test]
fn c_echo_semicolon() {
    let dir = tmpdir();
    let output = run_c("echo a; echo b", &dir);
    assert_ok(&output, "c_echo_semicolon");
    assert_eq!(
        str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
}

#[test]
fn c_mkdirat_openat2() {
    let dir = tmpdir();
    let script = concat!(
        "builtin mkdirat --mode 0755 d %>%d; ",
        "builtin openat2 --dirfd %d --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 f %>%f; ",
        "echo hello >%f",
    );
    let output = run_c(script, &dir);
    assert_ok(&output, "c_mkdirat_openat2");

    let content = std::fs::read_to_string(dir.join("d").join("f")).unwrap();
    assert_eq!(content.trim(), "hello");
}

#[test]
fn c_pipeline() {
    let dir = tmpdir();
    let output = run_c("echo pipedata | cat", &dir);
    assert_ok(&output, "c_pipeline");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "pipedata");
}

#[test]
fn c_nonzero_exit() {
    let dir = tmpdir();
    let output = run_c("nonexistent_xyzzy", &dir);
    assert!(
        !output.status.success(),
        "expected non-zero exit for unknown command"
    );
}

#[test]
fn c_and_operator_both_succeed() {
    let dir = tmpdir();
    let output = run_c("echo a && echo b", &dir);
    assert_ok(&output, "c_and_operator_both_succeed");
    let lines: Vec<&str> = str::from_utf8(&output.stdout).unwrap().lines().collect();
    assert_eq!(lines, ["a", "b"]);
}

#[test]
fn c_and_operator_short_circuit() {
    let dir = tmpdir();
    let output = run_c("nonexistent_xyz && echo should_not_run", &dir);
    assert!(!output.status.success(), "expected non-zero exit");
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout.trim(), "", "second command should not run");
}

#[test]
fn c_false_and_true() {
    let dir = tmpdir();
    let output = run_c("false && true", &dir);
    assert!(!output.status.success(), "false should exit non-zero");
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout.trim(), "", "true should not run after false");
}

#[test]
fn c_or_operator_short_circuit() {
    let dir = tmpdir();
    let output = run_c("true || echo should_not_run", &dir);
    assert_ok(&output, "c_or_operator_short_circuit");
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout.trim(), "", "second should not run after true");
}

#[test]
fn c_or_operator_runs_on_fail() {
    let dir = tmpdir();
    let output = run_c("false || echo ran", &dir);
    assert_ok(&output, "c_or_operator_runs_on_fail");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "ran");
}

#[test]
fn c_or_and_chain() {
    let dir = tmpdir();
    let output = run_c("false || echo ran && echo also_ran", &dir);
    assert_ok(&output, "c_or_and_chain");
    let lines: Vec<&str> = str::from_utf8(&output.stdout).unwrap().lines().collect();
    assert_eq!(lines, ["ran", "also_ran"]);
}

#[test]
fn c_or_quoted() {
    let dir = tmpdir();
    let output = run_c("echo \"a || b\"", &dir);
    assert_ok(&output, "c_or_quoted");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "a || b");
}

#[test]
fn c_and_operator_quoted() {
    let dir = tmpdir();
    let output = run_c("echo \"a && b\"", &dir);
    assert_ok(&output, "c_and_operator_quoted");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "a && b");
}

#[test]
fn c_and_operator_chained() {
    let dir = tmpdir();
    let output = run_c("echo a && echo b && echo c", &dir);
    assert_ok(&output, "c_and_operator_chained");
    let lines: Vec<&str> = str::from_utf8(&output.stdout).unwrap().lines().collect();
    assert_eq!(lines, ["a", "b", "c"]);
}

#[test]
fn c_and_operator_chain_short_circuit() {
    let dir = tmpdir();
    let output = run_c("echo a && nonexistent_xyz && echo c", &dir);
    assert!(!output.status.success(), "expected non-zero exit");
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout.trim(), "a", "only first command should run");
}

#[test]
fn c_empty_string() {
    let dir = tmpdir();
    let output = run_c("", &dir);
    assert_ok(&output, "c_empty_string");
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "");
}

#[test]
fn become_builtin_echo() {
    let dir = tmpdir();
    let output = run_c("become builtin echo hello", &dir);
    assert_ok(&output, "become_builtin_echo");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "hello");
}

#[test]
fn become_external_echo() {
    let dir = tmpdir();
    let output = run_c("become echo hello", &dir);
    assert_ok(&output, "become_external_echo");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "hello");
}

#[test]
fn become_no_args() {
    let dir = tmpdir();
    let output = run_c("become", &dir);
    assert!(!output.status.success(), "become with no args should fail");
}

#[test]
fn become_builtin_no_name() {
    let dir = tmpdir();
    let output = run_c("become builtin", &dir);
    assert!(
        !output.status.success(),
        "become builtin with no name should fail"
    );
}

#[test]
fn become_redirect() {
    let dir = tmpdir();
    let output = run_c("become builtin echo hello >out.txt", &dir);
    assert_ok(&output, "become_redirect");
    let content = std::fs::read_to_string(dir.join("out.txt")).unwrap();
    assert_eq!(content.trim(), "hello");
}

#[test]
fn become_prevents_continuation() {
    let dir = tmpdir();
    // `become` replaces the shell; subsequent commands never run.
    let output = run_c("become builtin echo hello; echo world", &dir);
    assert_ok(&output, "become_prevents_continuation");
    let stdout = str::from_utf8(&output.stdout).unwrap().trim();
    assert_eq!(stdout, "hello");
}

#[test]
fn builtin_pipe_create() {
    let dir = tmpdir();
    let output = run_c("builtin pipe %rd>%r %wr>%w", &dir);
    assert_ok(&output, "builtin_pipe_create");
}

#[test]
fn builtin_pipe_write_read() {
    let dir = tmpdir();
    let cmd = concat!(
        "builtin pipe %rd>%r %wr>%w; ",
        "builtin echo hello >%w; ",
        "builtin echo world >%w; ",
        "unset %w; ",
        "cat <%r",
    );
    let output = run_c(cmd, &dir);
    assert_ok(&output, "builtin_pipe_write_read");
    let stdout = str::from_utf8(&output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, ["hello", "world"]);
}

#[test]
fn builtin_pipe_nonblock() {
    let dir = tmpdir();
    let output = run_c("builtin pipe --flags O_NONBLOCK %rd>%r %wr>%w", &dir);
    assert_ok(&output, "builtin_pipe_nonblock");
}

#[test]
fn builtin_resolve_success() {
    let dir = tmpdir();
    let output = run_c("builtin resolve echo %>%fd", &dir);
    assert_ok(&output, "builtin_resolve_success");
}

#[test]
fn builtin_resolve_and_exec() {
    let dir = tmpdir();
    let cmd = concat!(
        "builtin resolve echo %>%fd; ",
        "builtin exec_fd %fd echo hello",
    );
    let output = run_c(cmd, &dir);
    assert_ok(&output, "builtin_resolve_and_exec");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "hello");
}

#[test]
fn builtin_echo_simple() {
    let dir = tmpdir();
    let output = run_c("builtin echo hello", &dir);
    assert_ok(&output, "builtin_echo_simple");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "hello");
}

#[test]
fn builtin_echo_empty() {
    let dir = tmpdir();
    let output = run_c("builtin echo", &dir);
    assert_ok(&output, "builtin_echo_empty");
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), "\n");
}

#[test]
fn builtin_echo_multiple() {
    let dir = tmpdir();
    let output = run_c("builtin echo hello world", &dir);
    assert_ok(&output, "builtin_echo_multiple");
    assert_eq!(
        str::from_utf8(&output.stdout).unwrap().trim(),
        "hello world"
    );
}

#[test]
fn builtin_echo_redirect_file() {
    let dir = tmpdir();
    let output = run_c("builtin echo data >out.txt", &dir);
    assert_ok(&output, "builtin_echo_redirect_file");
    let content = std::fs::read_to_string(dir.join("out.txt")).unwrap();
    assert_eq!(content.trim(), "data");
}

#[test]
fn builtin_echo_semicolon() {
    let dir = tmpdir();
    let output = run_c("builtin echo a; builtin echo b", &dir);
    assert_ok(&output, "builtin_echo_semicolon");
    let lines: Vec<&str> = str::from_utf8(&output.stdout).unwrap().lines().collect();
    assert_eq!(lines, ["a", "b"]);
}

#[test]
fn builtin_nonexistent() {
    let dir = tmpdir();
    let output = run_c("builtin nonexistent", &dir);
    assert_eq!(
        output.status.code(),
        Some(ENOSYS),
        "nonexistent builtin should exit with ENOSYS"
    );
}

#[test]
fn builtin_exec_at_script() {
    let dir = tmpdir();
    let cmd = concat!(
        "builtin mkdirat --mode 0755 foo %>%foo; ",
        "builtin openat2 --dirfd %foo --flags O_CREAT --flags O_RDWR --mode 0755 bar %>%bar; ",
        "builtin echo \"#!/bin/sh\" >%bar; ",
        "echo \"echo hello-world\" >%bar; ",
        "unset %bar; ",
        "builtin exec_at %foo bar",
    );
    let output = run_c(cmd, &dir);
    assert_ok(&output, "builtin_exec_at_script");
    assert_eq!(
        str::from_utf8(&output.stdout).unwrap().trim(),
        "hello-world"
    );
}

#[test]
fn readme_first_example() {
    let dir = tmpdir();

    let script = r#"
builtin mkdirat --dirfd %CWD --mode 0755 foo %>%foo
builtin mkdirat --dirfd %CWD --mode 0755 bar %>%bar
builtin openat2 --dirfd %foo --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 baz %>%baz
builtin renameat2 --olddirfd %foo --newdirfd %bar baz qux
echo "test" >%baz
"#;

    let output = run_fdshell(script, &dir);

    assert!(
        output.status.success(),
        "exit: {:?}\nstderr: {}",
        output.status.code(),
        str::from_utf8(&output.stderr).unwrap()
    );

    let target = dir.join("bar").join("qux");
    assert!(target.exists(), "bar/qux should exist after renameat2");
    let content = std::fs::read_to_string(&target).unwrap();
    assert_eq!(content.trim(), "test");
}

#[test]
fn file_redirects_and_pipeline() {
    let dir = tmpdir();

    // Pipeline: builtin echo writes to pipe, cat reads from pipe to file.
    // Using `builtin` ensures the child exits normally (preserving coverage).
    let script = r#"
echo "line1" >a.txt
echo "line2" >>a.txt
cat <a.txt >b.txt
builtin echo "pipedata" | cat >result.txt
"#;

    let output = run_fdshell(script, &dir);
    assert!(
        output.status.success(),
        "exit: {:?}\nstderr: {}",
        output.status.code(),
        str::from_utf8(&output.stderr).unwrap()
    );

    let content = std::fs::read_to_string(dir.join("result.txt")).unwrap();
    assert_eq!(content.trim(), "pipedata");
    assert_eq!(
        std::fs::read_to_string(dir.join("b.txt"))
            .unwrap()
            .lines()
            .count(),
        2,
    );
}

#[test]
fn nested_simple_echo() {
    let dir = tmpdir();
    let fdshell_dir = std::path::Path::new(BIN).parent().unwrap();
    let mut cmd = std::process::Command::new(BIN);
    cmd.args(["-c", "fdshell -c \"echo hello\""]);
    cmd.current_dir(&dir);
    let path = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{}:{}", fdshell_dir.display(), path));
    let output = cmd.output().unwrap();
    assert_ok(&output, "nested_simple_echo");
    assert_eq!(std::str::from_utf8(&output.stdout).unwrap().trim(), "hello");
}

#[test]
fn nested_import_export_roundtrip() {
    let dir = tmpdir();

    let script = concat!(
        "builtin openat2 --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 testfile %>%testfile; ",
        "fdshell -c \"builtin import_fd 5 %>%imported; builtin export_fd exported %imported\" ",
        "5>%testfile %>%captured; ",
        "builtin echo hello >%captured",
    );

    let fdshell_dir = std::path::Path::new(BIN).parent().unwrap();
    let mut cmd = std::process::Command::new(BIN);
    cmd.args(["-c", script]);
    cmd.current_dir(&dir);
    let path = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{}:{}", fdshell_dir.display(), path));
    let output = cmd.output().unwrap();
    assert_ok(&output, "nested_import_export_roundtrip");

    let content = std::fs::read_to_string(dir.join("testfile")).unwrap();
    assert_eq!(content.trim(), "hello");
}

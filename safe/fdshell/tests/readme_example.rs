#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

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

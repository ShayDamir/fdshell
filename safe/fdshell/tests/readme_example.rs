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
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
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

    assert!(output.status.success(), "exit: {:?}\nstderr: {}",
        output.status.code(),
        str::from_utf8(&output.stderr).unwrap());

    let target = dir.join("bar").join("qux");
    assert!(target.exists(), "bar/qux should exist after renameat2");
    let content = std::fs::read_to_string(&target).unwrap();
    assert_eq!(content.trim(), "test");
}

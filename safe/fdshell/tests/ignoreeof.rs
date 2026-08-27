#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn eof_exits_when_ignoreeof_is_off() {
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"set +o ignoreeof\n")
        .unwrap();
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn eof_is_ignored_when_ignoreeof_is_on() {
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Piped stdin is not a tty, so `ignoreeof` starts off; enable it
    // explicitly, then close stdin: the shell must stay alive and hint
    // how to leave.
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"set -o ignoreeof\n")
        .unwrap();
    drop(child.stdin.take().unwrap());
    let mut data = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while data.windows(19).all(|w| w != b"use `exit' to leave")
        && std::time::Instant::now() < deadline
    {
        let n = child.stdout.as_mut().unwrap().read(&mut buf).unwrap();
        assert!(n > 0, "shell exited on EOF: {data:?}");
        data.extend_from_slice(buf.get(..n).unwrap());
    }
    assert!(
        data.windows(19).any(|w| w == b"use `exit' to leave"),
        "no ignoreeof hint: {data:?}"
    );
    assert!(child.try_wait().unwrap().is_none(), "shell died on EOF");
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn ignoreeof_defaults_on_when_stdin_is_a_tty() {
    // `script` (util-linux) attaches a pty, so fdshell's stdin is a terminal
    // and the default-on `ignoreeof` must show up in `$-`.
    let mut child = Command::new("script")
        .args(["-qec", BIN, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"builtin echo [$-]\nexit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let out = str::from_utf8(&output.stdout).unwrap();
    assert!(
        out.contains("[i]"),
        "tty default ignoreeof missing from $-: {out:?}"
    );
}

#[test]
fn ignoreeof_appears_in_dollar_dash_when_on() {
    let (out, _err, code) = run("set -o ignoreeof; builtin echo [$-]");
    assert_eq!(code, 0);
    assert_eq!(out, "[i]\n");
    let (out, _err, code) = run("set -o ignoreeof; set +o ignoreeof; builtin echo [$-]");
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n");
}

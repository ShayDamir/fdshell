#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// The `ignoreeof` hint: the only observable proving the option took effect.
const HINT: &str = "use `exit' to leave";

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

/// The REPL on piped stdin: write `input`, close stdin, capture everything.
fn run_pipe(input: &str) -> (String, String, i32) {
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
        .write_all(input.as_bytes())
        .unwrap();
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// The REPL under a pty (`script -qec <bin>`, so `isatty(0)` is true): write
/// `input`, close the pty's stdin (that does deliver a pty EOF), settle, then
/// report stdout, the exit code (`None` while the shell is alive) and whether
/// it was.
///
/// No stdout read loop: after the hint a pty with closed stdin delivers
/// nothing, so a blocking `read` hangs until nextest's 10 s slow-timeout.
/// Sleep → `try_wait` → `kill` → drain is the shape that bounds the output.
/// The pty echoes the input and inserts `\r\n`, so assertions `contains` and
/// count matches, never exact stdout equality.
fn run_tty(input: &str) -> (String, Option<i32>, bool) {
    let mut child = Command::new("script")
        .args(["-qec", BIN, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if !input.is_empty() {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take().unwrap());
    std::thread::sleep(std::time::Duration::from_millis(500));
    let alive = child.try_wait().unwrap().is_none();
    if alive {
        let _ = child.kill();
    }
    let output = child.wait_with_output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        output.status.code(),
        alive,
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

/// The task #291 fix: a pipe's EOF is sticky, so `ignoreeof` (which guards the
/// interactive loop only) hints once per EOF read and then exits 0 — bash
/// exits 0 on the same stdin, silently.
#[test]
fn eof_on_pipe_hints_once_and_exits() {
    let (out, err, code) = run_pipe("set -o ignoreeof\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.matches(HINT).count(), 1, "stdout: {out:?}");
    assert!(err.is_empty(), "stderr={err}");
}

#[test]
fn short_i_flag_turns_ignoreeof_on() {
    // `set -i` is the short form of `set -o ignoreeof` (task #231): the same
    // hint, the same single-shot exit — the spelling must not spin.
    let (out, err, code) = run_pipe("set -i\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.matches(HINT).count(), 1, "stdout: {out:?}");
    assert!(err.is_empty(), "stderr={err}");
}

/// A tty delivers EOF as an event and then blocks, so `ignoreeof` (on by
/// default there, from the tty seed) keeps the shell alive after one hint:
/// the pinned output is `fdshell> use \`exit' to leave\r\nfdshell> ` and
/// nothing more.
#[test]
fn eof_on_terminal_keeps_shell_alive() {
    let (out, code, alive) = run_tty("");
    assert!(alive, "shell exited at tty EOF with code {code:?}: {out:?}");
    assert_eq!(out.matches(HINT).count(), 1, "stdout: {out:?}");
}

/// The terminal arm of the `ignoreeof`-off row: `set +o ignoreeof` turns the
/// tty default off and EOF exits (rc 0, `script` propagates the child's
/// status), with no hint at all.
#[test]
fn eof_off_on_terminal_exits() {
    let (out, code, alive) = run_tty("set +o ignoreeof\n");
    assert!(!alive, "shell alive at tty EOF with ignoreeof off: {out:?}");
    assert_eq!(code, Some(0), "stdout: {out:?}");
    assert_eq!(out.matches(HINT).count(), 0, "stdout: {out:?}");
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

#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// Spawn the REPL (no `-c`), pipe `input` to stdin, close it, and capture
/// stdout/stderr/exit code.
fn run_repl(input: &str) -> (String, String, i32) {
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

/// A single input line beyond the size cap must fail with the size error
/// instead of growing the line buffer until the process runs out of memory.
#[test]
fn oversize_single_line_is_capped() {
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Write exactly cap + 1 bytes in 4 KiB chunks: no 64 MiB allocation in
    // this process (nextest caps the test VA at 128 MB), and every byte is
    // consumed by the child before it bails, so the final write cannot hit
    // EPIPE after the child exits.
    let total = 64 * 1024 * 1024 + 1;
    let chunk = [b'a'; 4096];
    let mut left = total;
    while left > 0 {
        let n = core::cmp::min(left, chunk.len());
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(chunk.get(..n).unwrap())
            .unwrap();
        left -= n;
    }
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        !output.status.success(),
        "an oversize line must fail the REPL run"
    );
    assert!(
        stderr.contains("line exceeds the size limit"),
        "expected the size-limit error, stderr={stderr}"
    );
}

/// A line of exactly the cap (64 MiB) must be accepted: the limit allows
/// `limit` bytes and fails only on the next one, so this pins the boundary
/// the oversize test cannot (it pipes cap + 1, which every comparison
/// operator rejects).
#[test]
fn line_at_cap_is_accepted() {
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Write exactly cap bytes — a comment line (`#` + cap - 1 `a`s) — in 4
    // KiB chunks (no 64 MiB allocation in this process under the 128 MB
    // test VA cap), then `exit 0`. The line is a no-op once accepted, so
    // the run ends cleanly; a 64 MiB command line could not finish under
    // the cap (line buffer + script copy fill the child's VA, and its
    // "not found" report would be 134 MB). What this pins is the cap
    // check itself: under a `>=`/`==` mutant the child bails on the cap-th
    // byte with the size-limit error before any of that.
    let cap = 64 * 1024 * 1024;
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(b"#").unwrap();
    let chunk = [b'a'; 4096];
    let mut left = cap - 1;
    while left > 0 {
        let n = core::cmp::min(left, chunk.len());
        stdin.write_all(chunk.get(..n).unwrap()).unwrap();
        left -= n;
    }
    stdin.write_all(b"\nexit 0\n").unwrap();
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        !stderr.contains("line exceeds the size limit"),
        "a line of exactly the cap must be accepted, stderr={stderr}"
    );
}

#[test]
fn if_block_spans_lines() {
    let (out, err, code) = run_repl("if true; then\necho in-if\nfi\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("in-if"), "output: {out:?}");
    assert!(!err.contains("parse error"), "stderr: {err:?}");
    assert!(out.contains("> "), "continuation prompt missing: {out:?}");
}

#[test]
fn heredoc_spans_lines() {
    let (out, _err, code) = run_repl("cat <<EOF\nhello\nworld\nEOF\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("hello\nworld"), "output: {out:?}");
}

#[test]
fn function_block_spans_lines() {
    let (out, _err, code) = run_repl("f() {\necho from-f\n}\nf\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("from-f"), "output: {out:?}");
}

#[test]
fn while_loop_spans_lines() {
    let (out, _err, code) =
        run_repl("i=0\nwhile [ $i -lt 2 ]; do\ni=$((i+1))\ndone\nbuiltin echo done\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("done"), "output: {out:?}");
}

#[test]
fn case_block_spans_lines() {
    let (out, _err, code) =
        run_repl("x=foo\ncase $x in\nfoo) builtin echo is-foo ;;\nesac\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("is-foo"), "output: {out:?}");
}

#[test]
fn trailing_and_and_spans_lines() {
    let (out, err, code) = run_repl("echo a &&\necho b\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("a"), "output: {out:?}");
    assert!(out.contains("b"), "output: {out:?}");
    assert!(!err.contains("parse error"), "stderr: {err:?}");
}

#[test]
fn unbalanced_quote_spans_lines() {
    let (out, _err, code) = run_repl("echo \"a\nb\"\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("a\nb"), "output: {out:?}");
}

#[test]
fn single_line_unchanged() {
    let (out, _err, code) = run_repl("echo hi\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("hi"), "output: {out:?}");
}

#[test]
fn eof_mid_heredoc_executes_buffer() {
    // EOF mid-heredoc (ignoreeof off): the buffer is executed and reports the
    // missing-delimiter parse error.
    let (_out, err, _code) = run_repl("cat <<EOF\nbody\n");
    assert!(err.contains("terminating"), "stderr: {err:?}");
}

#[test]
fn eof_mid_if_executes_buffer() {
    // EOF mid-if (ignoreeof off): the buffer is executed and reports the
    // parse error.
    let (_out, err, _code) = run_repl("if true; then\n");
    assert!(err.contains("parse error"), "stderr: {err:?}");
}

#[test]
fn ignoreeof_mid_continuation_keeps_shell_alive() {
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Piped stdin is not a tty, so `ignoreeof` starts off; enable it
    // explicitly, start an if-block, then close stdin: the shell must stay
    // alive and hint how to leave.
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"set -o ignoreeof\nif true; then\n")
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
fn if_block_spans_lines_under_tty() {
    // `script` (util-linux) attaches a pty, so fdshell's stdin is a terminal.
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
        .write_all(b"if true; then\necho in-if\nfi\nexit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let out = str::from_utf8(&output.stdout).unwrap();
    assert!(out.contains("in-if"), "output: {out:?}");
    assert!(out.contains("> "), "continuation prompt missing: {out:?}");
}

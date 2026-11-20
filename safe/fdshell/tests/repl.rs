#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::str;
use std::time::{Duration, Instant};
use sys::getpwuid::getpwuid;
use sys::poll::{POLLIN, PollFd, poll};
use sys::unistd::{geteuid, gethostname};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// The prompt the REPL draws on a **terminal**: fdshell's own default, the bash
/// 5.3.9 default `PS1` string (measured 2026-11-18) **rendered** through the
/// shell's value provider (task #617) — the program name the REPL seeds, the
/// crate's own version, and the uid's character for a non-root runner (bash draws
/// its own identity there, `bash-5.3$ `; the README's prompt-time-expansion
/// paragraph words that divergence). Pinned
/// byte-for-byte as the counter the pty rows below use, so they pin one prompt per
/// read and their counts stayed put when the escapes started answering values.
const PROMPT: &str = concat!("fdshell-", env!("CARGO_PKG_VERSION"), "$ ");

/// The continuation prompt on a terminal: bash's `PS2` default, the string the
/// pty rows count. It is fdshell's own since before epic #197, byte-identical.
const CONT: &str = "> ";

/// The `ignoreeof` hint printed at an ignored EOF (task #291).
const HINT: &str = "use `exit' to leave";

/// The bounded event a pty row decides on: the row's shell exiting (rows that
/// end with `exit`), or `needle` landing on stdout while the shell is still
/// alive (EOF rows, where the tty-default `ignoreeof` keeps the loop reading).
/// A wall-clock budget instead of one of these is what makes a row load-flaky
/// (LESSONS: "A mutant \"caught\" by a flaky test is a false kill — re-run the
/// gate honestly after fixing the test").
enum Event {
    Exit,
    Seen(&'static str),
}

/// The event loop's bound — the repo's documented one (`tests/ignoreeof.rs`):
/// strictly under nextest's 10 s `slow-timeout`.
const SETTLE: Duration = Duration::from_secs(5);

/// The `poll` timeout, i.e. the event loop's cadence.
const POLL_STEP_MS: i32 = 50;

/// Bytes one poll-guarded read takes; prompt lines are far shorter.
const CHUNK: usize = 1024;

/// Poll std's own stdout pipe for [`POLL_STEP_MS`] and append what a
/// poll-guarded read yields: readiness means data or EOF is available, so the
/// read cannot block, and nothing is read after the event. The fd is std's own
/// pipe, borrowed for one `poll`, so no fd-type rule is bent (STYLE §5.1).
fn poll_into(child: &mut Child, observed: &mut Vec<u8>) {
    let Some(pipe) = child.stdout.as_mut() else {
        return;
    };
    let mut pfd = [PollFd::new(pipe.as_raw_fd(), POLLIN)];
    if poll(&mut pfd, POLL_STEP_MS).unwrap() == 0 {
        return;
    }
    let mut buf = vec![0u8; CHUNK];
    let read = pipe.read(&mut buf).unwrap();
    buf.truncate(read);
    observed.append(&mut buf);
}

/// The prompt tally of pty output: `(primary, continuation)` counts. Typed
/// lines never contain either needle, so the counts are the shell's own
/// prompts — the pty's echo is what rules out exact equality instead.
fn tally(out: &str) -> (usize, usize) {
    (out.matches(PROMPT).count(), out.matches(CONT).count())
}

/// The pty shape of a line buffered until **end of input**: the primary prompt
/// for the shape line, at least one continuation prompt, and the `ignoreeof`
/// hint exactly once. Past the hint nothing is deterministic (the terminal loop
/// prompts again and blocks in `read`), so nothing beyond `contains` is
/// pinned; the read-count pins live in the rows that end with `exit`.
fn assert_buffered_until_eof(out: &str) {
    assert!(out.contains(PROMPT), "primary prompt missing: {out:?}");
    assert!(
        out.contains(CONT),
        "the first line was dispatched, not buffered: {out:?}"
    );
    assert_eq!(out.matches(HINT).count(), 1, "hint count: {out:?}");
}

/// The pty shape of a line answered **at once**: no continuation prompt at all
/// (the pin — the shape never enters the buffer), a primary prompt, and the
/// `ignoreeof` hint once at end of input. The primary count past the hint is
/// racy, so it is not pinned.
fn assert_answered_at_once(out: &str) {
    assert!(out.contains(PROMPT), "primary prompt missing: {out:?}");
    assert_eq!(out.matches(CONT).count(), 0, "continuation prompt: {out:?}");
    assert_eq!(out.matches(HINT).count(), 1, "hint count: {out:?}");
}

/// The REPL under a pty (`script -qec <bin>`, so `isatty(0)` is true): write
/// `input`, close the pty's stdin (that does deliver a pty EOF), wait for
/// `event`, then `kill` a live child, then drain. Reports the captured stream,
/// the exit code (`None` while the shell is alive) and whether it was.
///
/// The child's stdout **and** stderr both go to the pty, so the returned stream
/// is the two of them as the pty saw them, plus the echo of the input and the
/// pty's `\r\n`; stderr-only assertions read this stream. Assert `contains` and
/// count matches, never exact equality. The wait is the event, never a fixed
/// sleep (LESSONS: "Test tty-gated behavior from safe-crate integration tests
/// via `script`"); `tests/ignoreeof.rs::run_tty` is the same shape.
fn run_tty(input: &str, event: Event) -> (String, Option<i32>, bool) {
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
        .write_all(input.as_bytes())
        .unwrap();
    drop(child.stdin.take().unwrap());
    let mut observed: Vec<u8> = Vec::new();
    if matches!(event, Event::Exit) {
        // The row ends with `exit`: the child's exit closes the pty's pipes, so
        // the drain itself is the event — no sleep, no kill.
        let output = child.wait_with_output().unwrap();
        return (
            str::from_utf8(&output.stdout).unwrap().to_string(),
            output.status.code(),
            false,
        );
    }
    if let Event::Seen(needle) = &event {
        let deadline = Instant::now() + SETTLE;
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            poll_into(&mut child, &mut observed);
            if String::from_utf8_lossy(&observed).contains(needle) {
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
        }
    }
    let alive = child.try_wait().unwrap().is_none();
    if alive {
        let _ = child.kill();
    }
    let output = child.wait_with_output().unwrap();
    let mut bytes = observed;
    bytes.extend_from_slice(&output.stdout);
    (
        str::from_utf8(&bytes).unwrap().to_string(),
        output.status.code(),
        alive,
    )
}

/// The pty shape of a row whose `PS1` has to be in place **before** its first
/// prompt: `script -qec "env PS1=<ps1> <bin>" /dev/null`, since an interactive
/// shell imports an inherited `PS1` (the row that pins the import itself is
/// [`an_inherited_env_value_reaches_the_prompt`], and the `env` form is why the
/// variable is set after the shell). `input` is written and the pty's stdin
/// closed; the row ends with `exit`, so the child's exit is the event — no sleep,
/// no kill (LESSONS). The stream is the same one `run_tty` returns: the pty's echo
/// of the input and its `\r\n` are part of it, so assert `contains` and counts.
fn run_tty_ps1(ps1: &str, input: &str) -> (String, Option<i32>) {
    let (bytes, code) = run_tty_ps1_bytes(ps1, input);
    (str::from_utf8(&bytes).unwrap().to_string(), code)
}

/// [`run_tty_ps1`]'s stream as **bytes**, for the row whose drawn prompt carries a
/// byte that is not valid UTF-8 — `\777`'s 0xff — and would panic the decode above.
fn run_tty_ps1_bytes(ps1: &str, input: &str) -> (Vec<u8>, Option<i32>) {
    let mut child = Command::new("script")
        .args(["-qec", &format!("env PS1='{ps1}' {BIN}"), "/dev/null"])
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
    (output.stdout, output.status.code())
}

/// Spawn the REPL (no `-c`), pipe `input` to stdin, close it, and capture
/// stdout/stderr/exit code. stdin is **not** a terminal, so no prompt is drawn
/// (bash draws none off-terminal either) and stdout is the body alone — the pty
/// rows are where the prompts are pinned, and these exact-stdout assertions
/// double as the non-tty-silence pin.
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
    // Piped stdin draws no prompt, so this row is body/stderr only; the
    // continuation prompt the buffering earns is pinned by
    // [`if_block_spans_lines_under_tty`].
    let (out, err, code) = run_repl("if true; then\necho in-if\nfi\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("in-if"), "output: {out:?}");
    assert!(!err.contains("parse error"), "stderr: {err:?}");
}

#[test]
fn heredoc_spans_lines() {
    let (out, _err, code) = run_repl("cat <<EOF\nhello\nworld\nEOF\nexit\n");
    assert_eq!(code, 0);
    assert!(out.contains("hello\nworld"), "output: {out:?}");
}

/// Task #322: a here-doc inside a **closed** single-line keyword block keeps
/// reading for its body lines (two continuation reads on a terminal) and the
/// buffer runs with the body attached — bash 5.3.9 prints `{a,b}`, rc 0 on
/// every row; on piped stdin nothing but the body is printed (no prompt
/// off-terminal), so the exact stdout pinned below is the body alone.
///
/// Measured against `bash --norc --noprofile` on the same stdin (its prompts
/// excluded): all six arm sites print `{a,b}` rc 0, and fdshell now matches
/// byte-for-byte behind its own prompts. `master` answered the first line as
/// complete: `here-doc: missing terminating delimiter line 'T'` plus
/// `"a" not found` + `"T" not found` as the body lines ran as statements.
/// The top-level twin stays pinned by [`heredoc_spans_lines`]. The `{ …; }`
/// group row joined the family with task **#315** (the group body claims its
/// region, the same rule as every block branch); its prompt shape is identical
/// to the keyword rows.
#[test]
fn single_line_block_heredoc_spans_lines() {
    for row in [
        "for x in a; do cat <<T; done",
        "if true; then cat <<T; fi",
        "while :; do cat <<T; break; done",
        "until false; do cat <<T; break; done",
        "case x in x) cat <<T;; esac",
        "if false; then :; else cat <<T; fi",
        "{ cat <<T; }",
    ] {
        let (out, err, code) = run_repl(&format!("{row}\n{{a,b}}\nT\nexit\n"));
        assert_eq!(code, 0, "{row}: stderr={err}");
        assert_eq!(out, "{a,b}\n", "{row}: stdout: {out:?}");
        assert!(err.is_empty(), "{row}: stderr={err}");
    }
}

/// Task **#332**: a here-doc statement **before** a `{ …; }` group and a
/// statement after the group's own `;`. The line carries two body regions, so the
/// read keeps buffering through four continuation reads — bash's continuation
/// count on the same stdin — and the buffer runs as `bodyX`, `a`, `bodyB` at
/// rc 0. Piped stdin draws no prompt, so the exact stdout is the body alone.
///
/// `master` stopped after two continuation reads (the line looked complete with
/// region 0 consumed) and then reported the body lines of region 1 as missing
/// commands, `"bodyX" not found` … ; with the group's resume fixed the trailing
/// statement is scanned where its own region lives. Measured byte-for-byte.
#[test]
fn pre_block_heredoc_behind_a_group_spans_both_regions() {
    let (out, err, code) = run_repl("cat <<X; { echo a; }; cat <<B\nbodyX\nX\nbodyB\nB\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "bodyX\na\nbodyB\n", "stdout: {out:?}");
    assert!(err.is_empty(), "stderr={err}");
}

/// Task #322's shape at the prompt, at **parity** with bash since task
/// **#364**.
///
/// The `elif` row at EOF: the `E` delimiter never arrives, the claim covers the
/// body lines, the buffer executes and prints `{a,b}\nT` with the end-of-file
/// warning, rc 0 — the script-mode twin is
/// `tests/heredoc.rs::cond_bodies_of_an_incomplete_line_stream_take_the_empty_tail`;
/// bash 5.3.9 prints the same with its own warning wording (measured
/// 2026-10-29). The group twin `{ cat <<T; }` flipped to bash parity with task
/// **#315** and is pinned in the family above.
#[test]
fn closed_block_heredoc_prompt_reaches_parity() {
    // EOF (no `exit`): the third continuation read consumes EOF, the buffer
    // executes. Piped stdin draws no prompt, so the body is the whole stdout.
    let (out, err, code) = run_repl("if false; then :; elif true; then cat <<E; fi\n{a,b}\nT\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "{a,b}\nT\n", "stdout: {out:?}");
    assert!(err.contains("delimited by end-of-file"), "stderr={err}");
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

/// A trailing plain `&` (task #326) is **complete** input, not a continuation:
/// the line executes at once (one read, one prompt — no `>` continuation
/// prompt), the job runs in the background, the prompt returns, and the next
/// line's `wait` reaps it.
#[test]
fn trailing_ampersand_is_complete_input() {
    let (out, err, code) = run_repl("echo hi &\nwait\nexit\n");
    assert_eq!(code, 0, "stderr: {err:?}");
    assert!(out.contains("hi"), "output: {out:?}");
    assert!(!err.contains("parse error"), "stderr: {err:?}");
    // The prompt-shape pin lives on a terminal (the pty's echo would make the
    // body assertion above meaningless there): the shape earns no continuation
    // prompt, and the reads — the `&` line, `wait`, `exit` — are three.
    let (tty, code, alive) = run_tty("echo hi &\nwait\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(!alive, "pty: {tty:?}");
    assert_eq!(tally(&tty), (3, 0), "prompts: {tty:?}");
    assert!(!tty.contains("parse error"), "pty: {tty:?}");
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

/// A blank line is not an input: the shell re-prompts and keeps reading, so
/// only end of input (`exit`, EOF) ends it. Prompt counts are read counts —
/// line, blank line, `exit` — and a shell that quit on the blank line prints
/// two; the count is pinned on a terminal, where prompts are drawn at all.
#[test]
fn blank_line_does_not_end_the_shell() {
    let (out, err, code) = run_repl("echo hi\n\nexit\n");
    assert_eq!(code, 0, "stderr: {err:?}");
    assert!(out.contains("hi"), "output: {out:?}");
    // The prompt count is the read counter, so it is a terminal assertion:
    // line, blank line, `exit`. A shell that quit on the blank line draws two.
    let (tty, code, _) = run_tty("echo hi\n\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (3, 0), "prompts: {tty:?}");
}

/// Whitespace-only input is blank too: it is skipped with a prompt and never
/// dispatched (dispatching `   ` reports a parse error on stderr).
#[test]
fn whitespace_line_is_skipped_not_executed() {
    let (out, err, code) = run_repl("   \necho hi\nexit\n");
    assert_eq!(code, 0, "stderr: {err:?}");
    assert!(out.contains("hi"), "output: {out:?}");
    assert!(!err.contains("parse error"), "stderr: {err:?}");
    // The skipped line is still a read, so a terminal shows three prompts:
    // the whitespace line, `echo hi`, `exit`.
    let (tty, code, _) = run_tty("   \necho hi\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (3, 0), "prompts: {tty:?}");
    assert!(!tty.contains("parse error"), "pty: {tty:?}");
}

#[test]
fn eof_mid_heredoc_executes_buffer() {
    // EOF mid-heredoc (ignoreeof off): the buffer is executed — the claim
    // covers the body, and the run carries the end-of-file warning; bash 5.3.9
    // measured 2026-10-29 prints `body` with its own warning wording (the
    // prompt bookkeeping above this test is fdshell's own, unchanged).
    let (_out, err, _code) = run_repl("cat <<EOF\nbody\n");
    assert!(err.contains("delimited by end-of-file"), "stderr: {err:?}");
}

#[test]
fn eof_mid_if_executes_buffer() {
    // EOF mid-if (ignoreeof off): the buffer is executed and reports the
    // parse error.
    let (_out, err, _code) = run_repl("if true; then\n");
    assert!(err.contains("parse error"), "stderr: {err:?}");
}

/// `ignoreeof` on a terminal, EOF mid-continuation: the terminal delivers one
/// EOF event, the hint prints and the shell stays alive at the continuation
/// prompt (measured shape: the primary prompt, the echoed input line, a
/// continuation prompt, the hint, then blocked — the pty's echo and `\r\n` are
/// part of the stream, so the row counts and contains, never matches exactly).
#[test]
fn ignoreeof_mid_continuation_keeps_shell_alive_on_terminal() {
    let (out, _code, alive) = run_tty("if true; then\n", Event::Seen(HINT));
    assert!(alive, "shell died at tty EOF mid-continuation: {out:?}");
    assert_eq!(
        out.matches(HINT).count(),
        1,
        "ignoreeof hint count: {out:?}"
    );
    // Both prompts precede the event, so they are deterministic; the loop
    // prompts again after it and blocks, so nothing past the hint is pinned.
    assert!(out.contains(PROMPT), "primary prompt missing: {out:?}");
    assert!(out.contains(CONT), "continuation prompt missing: {out:?}");
}

/// The same shape over a pipe ends the input: the continuation read hits EOF,
/// `ignoreeof` hints and the buffer is executed (the `eof_mid_if_executes_buffer`
/// parse error), then the next primary read hits the same EOF and hints again —
/// two hints, one per EOF read, and rc 0. A shell that kept reading the closed
/// pipe would print the hint forever (the task #291 spin).
#[test]
fn ignoreeof_mid_continuation_on_pipe_exits() {
    let (out, err, code) = run_repl("set -o ignoreeof\nif true; then\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(err.contains("parse error"), "stderr: {err:?}");
    assert_eq!(out.matches(HINT).count(), 2, "stdout: {out:?}");
}

#[test]
fn if_block_spans_lines_under_tty() {
    // `script` (util-linux) attaches a pty, so fdshell's stdin is a terminal
    // and the continuation prompt the buffering earns is drawn: bash's `PS2`
    // default, which epic #197 leaves byte-identical.
    let (out, code, _) = run_tty("if true; then\necho in-if\nfi\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {out:?}");
    assert!(out.contains("in-if"), "output: {out:?}");
    assert!(out.contains(CONT), "continuation prompt missing: {out:?}");
}

/// Task #236 §9.5 (plan §9.5): a closer written in a block body's **argument
/// position** (`while :; do echo x done`, `if a; then b fi`,
/// `for i in a; do echo done`, the glued-`esac` form) is word data, so the
/// block stays open and the shape reaches the REPL's `> ` continuation buffer
/// — where `master` answered the very first line (a parse error, or for
/// `while :; do echo x done` an accepted infinite loop).
///
/// Measured against **`bash --norc --noprofile -i`** on a pty
/// (`script -qec "bash --norc --noprofile -i" /dev/null`). Plain `bash -i` on
/// this box `exec`s fish (the user `~/.bashrc` replaces the shell at
/// interactive startup), so a `bash -i` probe reports fish's prompt and fish's
/// error texts, not bash's; `--norc --noprofile` is what reaches bash, and the
/// comparison is the **buffering and the joined-line result**, not the prompt
/// bytes. Under it bash buffers the same shapes, and running the buffer with
/// the closer on the next line prints bash's stdout verbatim:
/// `for i in a; do echo done` + `done` prints `done`, the `case` twin prints
/// `yes`, and `if a; then b fi` + `fi` runs the `then` body (`a` is
/// command-not-found in bash too), so `echo tail` prints. EOF mid-buffer
/// executes the buffer and reports the parse error, the same rule
/// [`eof_mid_if_executes_buffer`] pins; bash prints its
/// `unexpected end of file` report and exits 2 there, fdshell reports the
/// parse error and exits 0 — the documented REPL rc-1-vs-2 family (README,
/// task #294), not a divergence of these shapes.
#[test]
fn glued_closer_shapes_buffer_in_the_repl() {
    // The buffer joins the two lines into ONE dispatch: three prompts (the
    // shape, the joined buffer, `exit`) — a shell that ran each line by
    // itself would print four, and `master` did (its answer to line 1 is a
    // parse error, which is why these rows must not carry one).
    for (input, want, want_err) in [
        (
            "for i in a; do echo done\ndone\necho tail\nexit\n",
            "done\ntail\n",
            "",
        ),
        (
            "case x in x) echo yes;; *) echo no esac\nesac\necho tail\nexit\n",
            "yes\ntail\n",
            "",
        ),
        (
            "if a; then b fi\nfi\necho tail\nexit\n",
            "tail\n",
            "\"a\" not found",
        ),
    ] {
        // The joined buffer on a pipe is body-only (no prompt off-terminal),
        // so the exact stdout is the buffer's own output.
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr={err}");
        assert_eq!(out, want, "{input}: stdout: {out:?}");
        assert!(
            !err.contains("parse error"),
            "{input}: parse error: {err:?}"
        );
        assert!(
            want_err.is_empty() || err.contains(want_err),
            "{input}: stderr={err:?}"
        );
        // Three prompts (the shape, the joined buffer, `exit`) — a shell that
        // ran each line by itself would draw four, and `master` did (its answer
        // to line 1 is a parse error, which is why these rows must not carry
        // one); plus the single continuation prompt of the buffered read.
        let (tty, code, _) = run_tty(input, Event::Exit);
        assert_eq!(code, Some(0), "{input}: pty: {tty:?}");
        assert_eq!(tally(&tty), (3, 1), "{input}: prompts: {tty:?}");
        assert!(
            !tty.contains("parse error"),
            "{input}: pty parse error: {tty:?}"
        );
    }
    // EOF mid-buffer on a pipe: the buffer is executed and reports the parse
    // error — master's infinite loop (`while :; do echo x done` was accepted)
    // is a regression here, so the row is pinned with a closed stdin.
    for input in ["while :; do echo x done\n", "if a; then b fi\n"] {
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr={err}");
        assert!(out.is_empty(), "no prompt, no output: {out:?}");
        assert!(
            err.contains("parse error"),
            "{input}: the buffer was not executed: {err:?}"
        );
        // The same shape on a terminal: the first line is buffered (a
        // continuation prompt is drawn), and the tty's `ignoreeof` hint is the
        // event that bounds the row — the buffer is not run there.
        let (tty, code, alive) = run_tty(input, Event::Seen(HINT));
        assert!(alive, "{input}: pty: {tty:?}");
        assert_eq!(code, None, "{input}: pty: {tty:?}");
        assert_buffered_until_eof(&tty);
    }
}

/// Task **#306**: an unclosed `for i in done` is an **unclosed block**, not a
/// parse error on the first line. The list's closer-shaped word no longer ends
/// the block span, so the line is incomplete and the REPL buffers it under `> `
/// (task #294's buffering rule) until the closer arrives, and the completed
/// statement runs. `master` answered the first line with `expected 'do'` and
/// then ran `do` and `done` as commands of their own (the two
/// `failed to resolve command path` reports). At EOF the buffer is executed and
/// reports its missing closer — bash prints its `unexpected end of file` report
/// and exits 2, fdshell reports the parse error and exits 0 (the documented
/// REPL rc-1-vs-2 family, README).
#[test]
fn for_with_a_closer_shaped_list_word_buffers_until_the_closer() {
    // The completed statement, piped: body-only stdout, so the loop's own
    // output is the whole answer.
    let (out, err, code) = run_repl("for i in done\ndo echo \"$i\"; done\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(
        out, "done\n",
        "the loop iterates over the list word: {out:?}"
    );
    assert!(!err.contains("parse error"), "parse error: {err:?}");
    // Two prompts (the buffered line, `exit`) plus the one continuation prompt
    // that proves the first line was buffered, not dispatched.
    let (tty, code, _) = run_tty("for i in done\ndo echo \"$i\"; done\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (2, 1), "prompts: {tty:?}");

    // EOF mid-buffer on a pipe: the buffer is executed and reports the missing
    // closer, and nothing is printed to stdout (no prompt off-terminal).
    let (out, err, code) = run_repl("for i in done\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.is_empty(), "no prompt, no output: {out:?}");
    assert!(
        err.contains("'for' is missing its closing 'done'"),
        "the buffer was not executed as an unclosed block: {err:?}"
    );
    // The same shape on a terminal: the line is buffered (a continuation prompt
    // is drawn) and the tty's `ignoreeof` hint bounds the row.
    let (tty, code, alive) = run_tty("for i in done\nexit\n", Event::Seen(HINT));
    assert!(alive, "pty: {tty:?}");
    assert_eq!(code, None, "pty: {tty:?}");
    assert_buffered_until_eof(&tty);
}

/// Task **#396**: the glued `for((…))` is a keyword word at the byte level
/// now, so the **byte scan opens the block frame** and the REPL sees the same
/// buffering as any other unclosed block: the first line waits under `> ` for
/// the closer, and the completed statement runs (`0`, `1` — bash parity). At
/// EOF the buffer is executed and reports its missing closer — bash prints its
/// `unexpected end of file` report and exits 2, fdshell reports the parse error
/// and exits 0: the documented REPL rc-1-vs-2 family (README). `master`
/// dispatched each line by itself (`do`/`done` ran as commands).
#[test]
fn glued_arith_for_buffers_until_the_closer() {
    // The completed glued loop, piped: the two numbers are the whole stdout.
    let (out, err, code) = run_repl("for((i=0;i<2;i++))\ndo echo $i; done\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "0\n1\n", "the completed glued loop runs: {out:?}");
    assert!(!err.contains("parse error"), "parse error: {err:?}");
    // Two prompts (the buffered line, `exit`) plus the continuation prompt that
    // proves the first line was buffered, not dispatched.
    let (tty, code, _) = run_tty("for((i=0;i<2;i++))\ndo echo $i; done\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (2, 1), "prompts: {tty:?}");

    // EOF mid-buffer on a pipe: the buffer is executed and reports the missing
    // closer, naming the word the byte scan measured (`for((i=0`).
    let (out, err, code) = run_repl("for((i=0;i<2;i++))\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.is_empty(), "no prompt, no output: {out:?}");
    assert!(
        err.contains("'for((i=0' is missing its closing 'done'"),
        "the buffer was not executed as an unclosed block: {err:?}"
    );
    // The same shape on a terminal: buffered, then the `ignoreeof` hint.
    let (tty, code, alive) = run_tty("for((i=0;i<2;i++))\n", Event::Seen(HINT));
    assert!(alive, "pty: {tty:?}");
    assert_eq!(code, None, "pty: {tty:?}");
    assert_buffered_until_eof(&tty);
}

/// Task #294: an **unclosed mid-run construct** (a `{ …` cond part, a keyword
/// block after `&&` or behind `|`, a `true && f() {` definition) is skipped
/// whole inside its run, so the scan stamps its closedness on the statement
/// and the REPL buffers the line under `> ` until the closer arrives. At EOF
/// the buffer is executed and reports the missing closer — bash prints its
/// `unexpected end of file` report and exits 2, fdshell reports the parse
/// error and exits 0: the documented REPL rc-1-vs-2 family (README).
#[test]
fn unclosed_midrun_constructs_buffer_until_eof() {
    for (input, want) in [
        ("true && { echo g\n", "group is missing a closing brace"),
        ("true && while :; do echo hi\n", "expected 'done'"),
        ("echo a | while :; do echo hi\n", "expected 'done'"),
        (
            "true && f() {\n",
            "function definition is missing a closing brace",
        ),
    ] {
        // Piped (no prompt off-terminal): the buffer is executed at EOF and its
        // diagnostic is the whole answer — nothing of the shape is printed.
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr={err}");
        assert!(out.is_empty(), "{input}: stdout: {out:?}");
        assert!(err.contains(want), "{input}: stderr={err}");
        assert_eq!(
            err.matches(want).count(),
            1,
            "{input}: the diagnostic is reported once: {err}"
        );
        // On a terminal the same shape is buffered until end of input: the
        // primary prompt, then a continuation prompt, then the `ignoreeof` hint
        // — the counts past that event are racy, so the shape is pinned with
        // `contains`.
        let (tty, code, alive) = run_tty(input, Event::Seen(HINT));
        assert!(alive, "{input}: pty: {tty:?}");
        assert_eq!(code, None, "{input}: pty: {tty:?}");
        assert_buffered_until_eof(&tty);
    }
}

/// A **closed** multi-line mid-run block is buffered and runs as **one**
/// statement, the bash parity row of task #294: `true && while :; do` +
/// `echo hi` + `break` + `done` prints `hi` (the `break` is inside a loop, so
/// the lines never run apart), and the group twin prints its two lines.
#[test]
fn closed_midrun_construct_runs_as_one_statement() {
    // Piped rows: no prompt off-terminal, so the exact stdout is the buffer's
    // own output. The prompt tallies are the terminal rows'.
    let (out, err, code) = run_repl("true && while :; do\necho hi\nbreak\ndone\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert!(
        !err.contains("not inside a loop"),
        "the lines ran apart: {err}"
    );
    assert!(!err.contains("parse error"), "parse error: {err}");
    assert_eq!(out, "hi\n", "stdout: {out:?}");
    let (tty, code, _) = run_tty(
        "true && while :; do\necho hi\nbreak\ndone\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (2, 3), "prompts: {tty:?}");

    let (out, err, code) = run_repl("true && { echo g\necho h\n}\necho tail\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert!(!err.contains("parse error"), "parse error: {err}");
    assert_eq!(out, "g\nh\ntail\n", "stdout: {out:?}");
    let (tty, code, _) = run_tty(
        "true && { echo g\necho h\n}\necho tail\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (3, 2), "prompts: {tty:?}");
}

/// A closed **single-line** mid-run construct never prompts: the stamp is the
/// span's closedness, so a line whose closer is there runs at once — the
/// continuation prompt is the unclosed shape's alone (task #294); the absence
/// is pinned on a terminal, where prompts are drawn at all. Every loop
/// row is bounded by a `break` (a `while :; do :; done` twin would loop
/// forever in the REPL as it does in bash, and hang the suite).
#[test]
fn closed_single_line_midrun_never_prompts() {
    // `reads` is the row's read count on a terminal — one primary prompt per
    // read (the shape, the call the definition row needs, `exit`) — and the
    // continuation count is the pin: the shape never enters the buffer.
    for (input, want, reads) in [
        ("true && { echo g; }\nexit\n", "g", 2),
        ("true && while :; do echo w; break; done\nexit\n", "w", 2),
        // A definition writes no output of its own, so the call that follows
        // shows the mid-run `f() { … }` did define it.
        ("true && f() { echo F; }\nf\nexit\n", "F", 3),
    ] {
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr: {err}");
        assert!(!err.contains("parse error"), "parse error: {err}");
        assert!(out.contains(want), "{input}: stdout: {out:?}");
        let (tty, code, _) = run_tty(input, Event::Exit);
        assert_eq!(code, Some(0), "{input}: pty: {tty:?}");
        assert_eq!(tally(&tty), (reads, 0), "{input}: prompts: {tty:?}");
    }
    // The closed **stage** arm: its parse-plus-output oddity is #292's (the rc
    // and the `expected 'done'` family), so only the continuation prompt is
    // pinned here — the shape never enters the buffer, and the stage runs.
    let (out, _, _) = run_repl("echo a | while read l; do echo \"$l\"; done\nexit\n");
    assert!(out.contains('a'), "stdout: {out:?}");
    let (tty, code, _) = run_tty(
        "echo a | while read l; do echo \"$l\"; done\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (2, 0), "prompts: {tty:?}");
}

/// The documented divergence at EOF (README, "Interactive shell", task #294):
/// fdshell executes the buffer it has, so the statements written **before** an
/// unclosed mid-run construct run (`echo a` prints) and the construct then
/// reports its missing closer — where bash parses the whole line first, runs
/// nothing, and exits 2.
#[test]
fn statements_before_an_unclosed_midrun_run_at_eof() {
    // Piped (no prompt off-terminal): the prefix statement's own output is the
    // whole stdout, and the report is stderr's.
    let (out, err, code) = run_repl("echo a; true && { echo g\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "a\n", "the prefix statement ran: {out:?}");
    assert!(
        err.contains("group is missing a closing brace"),
        "stderr={err}"
    );
    // On a terminal the shape is buffered until end of input: primary prompt,
    // continuation prompt, `ignoreeof` hint.
    let (tty, code, alive) = run_tty("echo a; true && { echo g\n", Event::Seen(HINT));
    assert!(alive, "pty: {tty:?}");
    assert_eq!(code, None, "pty: {tty:?}");
    assert_buffered_until_eof(&tty);
}

/// Task **#401**: a run-start `{ …; }` whose **own** brace closed but whose tail
/// group did not joins the buffering family — what the prompt reads is the
/// **statement's** closedness, not the construct's own. Measured against
/// `bash --norc --noprofile -i` under `script -qec … /dev/null` (the convention of
/// the tests above): bash buffers the same line under `>` and running the buffer
/// with the closer on the next line prints `a`, `b`, `tail`. At end of input bash
/// reports `syntax error: unexpected end of file from '{' command on line 1` and
/// exits 2, while fdshell runs what it can — the closed group's `echo a` prints —
/// and reports its missing closer, exiting 0: the documented REPL rc-1-vs-2 family
/// (README, task #294), and the reason the buffer still runs what it can.
#[test]
fn a_closed_group_with_an_unclosed_tail_group_buffers() {
    // Closer on the next line: the buffer joins the lines into ONE dispatch.
    // Piped (no prompts), so the exact stdout is the buffer's own output.
    let (out, err, code) = run_repl("{ echo a; } && { echo b\n}\necho tail\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "a\nb\ntail\n", "the joined buffer ran: {out:?}");
    assert!(!err.contains("parse error"), "parse error: {err:?}");
    // Three prompts on a terminal (the shape, the joined buffer, `exit`) plus
    // exactly one continuation prompt.
    let (tty, code, _) = run_tty("{ echo a; } && { echo b\n}\necho tail\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (3, 1), "prompts: {tty:?}");

    // EOF mid-buffer on a pipe: the buffer is executed, so the closed group's
    // `echo a` prints, and the missing closer is reported.
    let (out, err, code) = run_repl("{ echo a; } && { echo b\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "a\n", "the closed group ran: {out:?}");
    assert!(
        err.contains("group is missing a closing brace"),
        "the buffer was not executed as an unclosed group: {err:?}"
    );
    // The same shape on a terminal: buffered, then the `ignoreeof` hint.
    let (tty, code, alive) = run_tty("{ echo a; } && { echo b\n", Event::Seen(HINT));
    assert!(alive, "pty: {tty:?}");
    assert_eq!(code, None, "pty: {tty:?}");
    assert_buffered_until_eof(&tty);
}

/// A single-line `case` whose arm ends with the plain `&` and whose `esac` has
/// not arrived (task #354): the block-opener gate owns the run, so the shape
/// enters the `> ` continuation buffer instead of answering the first line with
/// the statement split's parse error. The buffer's job is reaped by the `wait`
/// line (LESSONS: an async pin ends with `wait`).
#[test]
fn ampersand_in_a_case_arm_buffers_until_the_closer_arrives() {
    let (out, err, code) = run_repl("case x in x) echo a & ;;\nesac\nwait\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("a"), "stdout: {out:?}");
    assert!(!err.contains("parse error"), "parse error: {err}");
    // The prompt shape on a terminal: three prompts — the shape, the joined
    // buffer, `exit` — one per read, and the one continuation prompt the
    // buffering earns.
    let (tty, code, _) = run_tty("case x in x) echo a & ;;\nesac\nwait\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (3, 1), "prompts: {tty:?}");
    assert!(!tty.contains("parse error"), "pty: {tty:?}");
}

/// A here-doc statement **behind** a plain `&` on the same line (task #313):
/// the read keeps buffering the four body lines (four continuation prompts on a
/// terminal — both bodies), the buffer runs with both bodies attached, and no
/// body line is ever treated as a statement (before the fix the suffix's body
/// lines answered as `y` and `B` "not found"). The background job is reaped by
/// the `wait` line, and the prompt count is one per read: the shape, `wait`,
/// `exit`.
#[test]
fn heredocs_on_both_sides_of_an_ampersand_span_their_lines() {
    let (out, err, code) = run_repl("cat <<A & cat <<B\nx\nA\ny\nB\nwait\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    // The bodies are the output (piped stdout is body-only); their order is the
    // job's, so never pinned.
    assert!(out.contains('x') && out.contains('y'), "stdout: {out:?}");
    assert!(
        !err.contains("not found"),
        "body line ran as a statement: {err}"
    );
    assert!(!err.contains("parse error"), "parse error: {err}");
    // The prompt shape on a terminal: three primary prompts (the shape, `wait`,
    // `exit`) and the four continuation prompts of the body lines — every body
    // line is read as data, never as a statement.
    let (tty, code, _) = run_tty("cat <<A & cat <<B\nx\nA\ny\nB\nwait\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tally(&tty), (3, 4), "prompts: {tty:?}");
    assert!(
        !tty.contains("not found"),
        "body line ran as a statement: {tty:?}"
    );
}

/// The REPL keeps its own whitespace-tail rule (task #492, plan §4.4): the
/// comment-tail spelling is complete input here, where bash buffers it, so this
/// row executes at once — and since task **#367** the operator with nothing behind
/// it reports at once too (fdshell's rc-1 message family; bash buffers the shape
/// and answers rc 1 without printing, so its REPL prints nothing). The next line
/// runs, so `b` prints and the REPL exits 0. The scan-level rule that ignores
/// comment text is pinned in `cond_blocks` (the script path prints nothing); two
/// granularities, one shape each, both pinned. Measured 2026-10-30.
#[test]
fn a_comment_tail_after_a_trailing_operator_is_complete_input() {
    let (out, err, code) = run_repl("false && #c\necho b\nexit\n");
    assert_eq!(code, 0, "stderr: {err:?}");
    assert!(out.contains("b"), "output: {out:?}");
    assert!(
        err.contains("expected command after"),
        "the dangling operator reports: {err:?}"
    );
}

/// The REPL buffers a dangling operator (the shared walk says "keep reading", task
/// #492), reaches the end of input, and running the buffer is where the report of
/// task **#367** fires: fdshell's own message at rc 1, nothing of the list printed.
/// The exit code is the REPL's own end-of-input answer, so only the report is this
/// rule's; both are asserted with `contains` because the prompts make exact stdout
/// unstable. Measured 2026-10-30 against bash 5.3.9 and this build.
#[test]
fn a_dangling_operator_at_the_end_of_the_repl_input_reports() {
    let (out, err, code) = run_repl("echo a &&\n");
    assert!(
        err.contains("expected command after"),
        "bash stops at rc 2: {err:?}"
    );
    assert!(
        !out.contains("a\n"),
        "the report fires before any part of the list runs: {out:?}"
    );
    assert_eq!(
        code, 0,
        "the REPL's own end-of-input answer; stderr: {err:?}"
    );
}

/// Task **#402**: a brace at the very end of input **is** an opener boundary
/// for the byte rule (`segment/group/word.rs::opens_at` counts the missing
/// byte), so both title shapes buffer under `> ` until the closer arrives —
/// bash 5.3.9 buffers them too (`script -qec "bash --norc --noprofile -i"`
/// measured): `{` + `echo a` + `}` and `true && {` + `echo a` + `}` print `a`
/// as **one** dispatch. At end of input fdshell executes the buffer it has and
/// reports its own missing-closer error at rc 0 — bash exits 2 naming `{`:
/// the documented REPL rc-1-vs-2 family (README). The run-start report is the
/// scan level's (`script/unclosed.rs`, which fires **before** the parse layer,
/// LESSONS task #394's note); the mid-run one is the parse layer's (task
/// #294's family). The empty-group row answers with fdshell's own `group is
/// empty` where bash reports `syntax error near unexpected token '}'`, and the
/// arithmetic-`for` row entered the group branch from the `done` frame, so its
/// EOF report now names `}`: `'for' is missing its closing '}'` (bash: the
/// same `unexpected end of file from '{'` wording, rc 2). The two guard rows
/// stay answered at once — an **argument-position** brace stays word data in
/// bash and here, the callers' command-position gate holding it.
#[test]
fn a_brace_at_the_end_of_input_buffers_in_command_position() {
    // Closer on the next line: the buffer joins the lines into ONE dispatch.
    // Piped rows are body-only (no prompt off-terminal); the pty rows pin the
    // buffering shape (primary prompt, continuation prompts, one hint).
    for input in ["{\necho a\n}\n", "true && {\necho a\n}\n"] {
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "{input}: stderr: {err}");
        assert!(!err.contains("parse error"), "parse error: {err}");
        assert_eq!(out, "a\n", "{input}: the joined buffer ran: {out:?}");
        let (tty, code, alive) = run_tty(input, Event::Seen(HINT));
        assert!(alive, "{input}: pty: {tty:?}");
        assert_eq!(code, None, "{input}: pty: {tty:?}");
        assert_buffered_until_eof(&tty);
    }
    // EOF mid-buffer on a pipe: the buffer is executed and reports its missing
    // closer, and stdout carries no prompt bytes.
    for (input, want) in [
        ("{\n", "'{' is missing its closing '}'"),
        ("true && {\n", "group is missing a closing brace"),
        ("{\n}\n", "group is empty"),
        (
            "for ((i=0;i<2;i++)) {\n",
            "'for' is missing its closing '}'",
        ),
    ] {
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "{input}: bash exits 2: stderr={err}");
        assert!(out.is_empty(), "{input}: stdout: {out:?}");
        assert!(err.contains(want), "{input}: stderr={err}");
        assert_eq!(
            err.matches(want).count(),
            1,
            "{input}: the diagnostic is reported once: {err}"
        );
        let (tty, code, alive) = run_tty(input, Event::Seen(HINT));
        assert!(alive, "{input}: pty: {tty:?}");
        assert_eq!(code, None, "{input}: pty: {tty:?}");
        assert_buffered_until_eof(&tty);
    }
    // The arithmetic-`for` with its closer on the next line keeps bash's answer
    // (it buffered on master too, via the `done` frame; now via the group
    // branch — the observable is unchanged, and bash prints `a` twice).
    let (out, err, code) = run_repl("for ((i=0;i<2;i++)) {\necho a\n}\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert!(!err.contains("parse error"), "parse error: {err}");
    assert_eq!(out, "a\na\n", "the loop ran its group body: {out:?}");
    let (tty, code, alive) = run_tty("for ((i=0;i<2;i++)) {\necho a\n}\n", Event::Seen(HINT));
    assert!(alive, "pty: {tty:?}");
    assert_eq!(code, None, "pty: {tty:?}");
    assert_buffered_until_eof(&tty);
    // The guard rows: bash answers an argument-position brace at once (and
    // so does fdshell — the byte rule's `None` arms at the callers), no
    // continuation prompt. The pty rows pin that absence; two prompts is the
    // shape's read plus the end-of-input read the terminal keeps going after.
    for (input, want) in [("echo {\n", "{\n"), ("{ echo a; } && echo {\n", "a\n{\n")] {
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "{input}: stderr: {err}");
        assert_eq!(out, want, "{input}: answered at once: {out:?}");
        let (tty, code, alive) = run_tty(input, Event::Seen(HINT));
        assert!(alive, "{input}: pty: {tty:?}");
        assert_eq!(code, None, "{input}: pty: {tty:?}");
        assert_answered_at_once(&tty);
    }
}

/// Task #406's shape through the interactive shell: the first line is complete and
/// runs, and the report fires on the **line-initial** line — the operator opens it, so
/// no command sits in front of it (bash prints `a` here too, then stops at rc 2 and runs
/// nothing of the rejected line). The reported line's own part never runs, the session
/// reads on to the end of input, and the exit code stays the REPL's own end-of-input
/// answer. Both are asserted with `contains` because the prompts make stdout unstable.
/// Measured 2026-11-03 against bash 5.3.9 (`bash` reading stdin) and this build.
#[test]
fn an_operator_opening_the_second_line_reports_and_the_session_reads_on() {
    let (out, err, code) = run_repl("echo a\n&& echo y\n");
    assert!(
        err.contains("expected command before"),
        "the line-initial operator reports: {err:?}"
    );
    assert!(
        out.contains("a\n"),
        "the complete line before it ran, as bash runs it: {out:?}"
    );
    assert!(
        !out.contains("y\n"),
        "the report fires before any part of the reported list runs: {out:?}"
    );
    assert_eq!(
        code, 0,
        "the REPL's own end-of-input answer; stderr: {err:?}"
    );
}

// ——— the prompt variables (epic #197), measured against bash 5.3.9 on 2026-11-18 ———

/// The default `PS1` on a terminal: bash 5.3.9's default string, **rendered**
/// through the shell's value provider — `fdshell-<version>$ ` for a non-root runner
/// (task #617). bash 5.3.9 draws its own identity on the same string, `bash-5.3$ `
/// (measured 2026-11-18); the README's prompt-time-expansion paragraph words that
/// divergence. One read, one prompt
/// — the typed line is `exit`, so the single match is the prompt itself. The `PS2`
/// default is byte-identical to fdshell's and is pinned by every continuation count
/// above.
#[test]
fn default_prompt_is_drawn_on_a_terminal() {
    let (tty, code, _) = run_tty("exit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches(PROMPT).count(), 1, "pty: {tty:?}");
}

/// `PS1` assigned as a command changes the **next** prompt: bash re-reads the
/// variable before every prompt. The `[p1] ` tally is the pty's echo of the
/// assignment line plus the one prompt drawn after it — a prompt resolved once
/// at startup would leave only the echo.
#[test]
fn an_assignment_changes_the_next_prompt() {
    let (tty, code, _) = run_tty("PS1='[p1] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[p1] ").count(), 2, "pty: {tty:?}");
    // The assignment line's own read is still prompted with the default.
    assert_eq!(tty.matches(PROMPT).count(), 1, "pty: {tty:?}");
}

/// The `PS2` twin: an incomplete construct is prompted with the assigned value.
/// The `| ` tally is the echo of the assignment line plus the single
/// continuation prompt; the three default prompts are the two reads that are
/// prompted with `PS1` before the buffer completes, and `exit`.
#[test]
fn an_assignment_changes_the_next_continuation_prompt() {
    let (tty, code, _) = run_tty("PS2='| '\nif true; then\nfi\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("| ").count(), 2, "pty: {tty:?}");
    assert_eq!(tty.matches(PROMPT).count(), 3, "pty: {tty:?}");
}

/// `export` goes through the same shared lookup as an assignment, so the shape
/// is the one [`an_assignment_changes_the_next_prompt`] pins.
#[test]
fn an_export_changes_the_next_prompt() {
    let (tty, code, _) = run_tty("export PS1='[e1] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[e1] ").count(), 2, "pty: {tty:?}");
    assert_eq!(tty.matches(PROMPT).count(), 1, "pty: {tty:?}");
}

/// `unset` returns the prompt to the bash default. The `[v]` tally is the echo
/// plus the one prompt drawn between the assignment and the unset (the word's
/// unquoted trailing space is not part of the value); the two default prompts
/// are the assignment's read and the `exit` read after the unset — the unset is
/// what makes the second one the default.
#[test]
fn unset_restores_the_default_prompt() {
    let (tty, code, _) = run_tty("PS1=[v]\nunset PS1\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[v]").count(), 2, "pty: {tty:?}");
    assert_eq!(tty.matches(PROMPT).count(), 2, "pty: {tty:?}");
}

/// A set-empty `PS1` draws **nothing** (measured against bash): the only default
/// prompt is the assignment line's own read, and the read of `exit` is prompted
/// with the empty value — i.e. not at all.
#[test]
fn a_set_empty_ps1_draws_nothing() {
    let (tty, code, _) = run_tty("PS1=\"\" \nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches(PROMPT).count(), 1, "pty: {tty:?}");
}

/// The `PS2` twin, measured the same way: the buffered read's prompt is empty,
/// so no continuation needle appears at all, and only the three primary prompts
/// (the two reads prompted with `PS1`, and `exit`) are drawn.
#[test]
fn a_set_empty_ps2_draws_nothing() {
    let (tty, code, _) = run_tty("PS2=\"\"\nif true; then\nfi\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches(CONT).count(), 0, "pty: {tty:?}");
    assert_eq!(tty.matches(PROMPT).count(), 3, "pty: {tty:?}");
}

/// A `PS1` inherited through the environment reaches the prompt through the same
/// shared lookup, and an interactive shell draws it (measured against bash
/// 5.3.9, which imports `PS1` from the environment for interactive shells). The
/// variable is set by `env` **inside** the pty command, not on `script`: a
/// non-interactive shell drops `PS1` from its child's environment (measured),
/// and `script` may run the command through one.
#[test]
fn an_inherited_env_value_reaches_the_prompt() {
    let mut child = Command::new("script")
        .args(["-qec", &format!("env PS1='[envp] ' {BIN}"), "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"exit\n").unwrap();
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let out = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(out.matches("[envp] ").count(), 1, "pty: {out:?}");
    assert!(
        !out.contains(PROMPT),
        "the default is not the answer here: {out:?}"
    );
}

// ——— the escape decoder (task #616), byte streams measured against bash 5.3.9
// on 2026-11-18; the inputs are raw strings (LESSONS) and only the decoder can
// produce the asserted bytes, since the pty echoes the typed line with the raw
// backslashes. None of these prompts contains a tally needle, so the counts in
// the rows above stay put. ———

/// A mechanical escape in the prompt: the decoder emits its byte (bash draws
/// the same, measured).
#[test]
fn a_bell_in_the_prompt_is_drawn_as_a_bell() {
    let (tty, code, _) = run_tty(concat!(r"PS1='a\ab'", "\nexit\n"), Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\x07b"), "pty: {tty:?}");
}

/// The three-digit octal rule, end to end.
#[test]
fn an_octal_escape_in_the_prompt_is_drawn_as_its_byte() {
    let (tty, code, _) = run_tty(concat!(r"PS1='a\101b'", "\nexit\n"), Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("aAb"), "pty: {tty:?}");
}

/// The `\xHH` extension (decision D1 — bash 5.3.9 measures it literal; the
/// README's prompt-time-expansion paragraph words that deviation).
#[test]
fn a_hex_escape_in_the_prompt_is_drawn_as_its_byte() {
    let (tty, code, _) = run_tty(concat!(r"PS1='a\x41b'", "\nexit\n"), Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("aAb"), "pty: {tty:?}");
}

/// The non-printing region: bash draws the region's bytes and consumes the
/// toggling markers (measured byte stream); the record's split structure is
/// pinned at unit level, where epic #198's contract lives.
#[test]
fn a_non_printing_region_is_drawn_and_recorded() {
    let (tty, code, _) = run_tty(
        concat!(r"PS1='a\[\e[31mB\e[0m\]C'", "\nexit\n"),
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\x1b[31mB\x1b[0mC"), "pty: {tty:?}");
}

// ——— the rest of the bash PROMPTING table (task #623): the table rows the
// sections above did not have, one row per escape, every expectation re-measured
// against bash 5.3.9 on 2026-11-20 (bash's bytes in each row's comment — tests
// never spawn bash, LESSONS: "Verify bash's real output before pinning a test, not
// the task text"). No row's prompt holds a tally needle, so the counts above stay
// put; every row is parity unless its comment points at the README's
// prompt-layer divergence paragraph. ———

/// `\e`: the decoder emits ESC (bash draws the same byte, measured).
#[test]
fn an_escape_in_the_prompt_is_drawn_as_a_escape() {
    let (tty, code) = run_tty_ps1(r"a\eb", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\x1bb"), "pty: {tty:?}");
}

/// `\n`: a real newline, which the pty renders as CR LF (bash measures the same,
/// measured). The row's needle is the byte the decoder wrote, not the pty's CR.
#[test]
fn a_newline_in_the_prompt_is_drawn_as_a_line_break() {
    let (tty, code) = run_tty_ps1(r"a\nb", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\r\nb"), "pty: {tty:?}");
}

/// `\r`: a real carriage return (bash draws the same byte, measured).
#[test]
fn a_carriage_return_in_the_prompt_is_drawn_as_a_carriage_return() {
    let (tty, code) = run_tty_ps1(r"a\rb", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\rb"), "pty: {tty:?}");
}

/// The `\\` escape arm, alone: one backslash — the pair is an escape here, not the
/// quote operator the row in the task #616 section pins (bash draws one backslash
/// too, measured).
#[test]
fn the_double_backslash_escape_is_drawn_as_one_backslash() {
    let (tty, code) = run_tty_ps1(r"a\\b", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains(r"a\b"), "pty: {tty:?}");
}

/// A decoded NUL: fdshell writes the byte into the prompt, so it reaches the pty
/// (`a\0b`); bash's prompt writer **drops** it and draws `ab` (measured — the
/// README's prompt-layer divergence paragraph). Not a writer bug: the record's data
/// contract and `a_nul_splits_the_run_and_is_pasted` are deliberate (task #616).
#[test]
fn a_decoded_nul_is_drawn_as_a_nul() {
    let (tty, code) = run_tty_ps1(r"a\000b", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\0b"), "pty: {tty:?}");

    // The 400 octal is the three-digit rule's overflow: 0o400 truncated to a byte
    // is the same NUL, and fdshell draws it (bash drops it, measured).
    let (tty, code) = run_tty_ps1(r"a\400b", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\0b"), "pty: {tty:?}");
}

/// `\D{…}`: the decoder does not know the brace-argument form and draws the escape
/// text (`a\D{%Y}b`); bash draws its strftime value (`a2026b`, measured).
/// D-623-2: documented, not implemented — it joins the README's prompt-layer
/// divergence paragraph; the pinned word is the decoder's own doc line.
#[test]
fn an_unrecognised_brace_escape_is_drawn_as_its_text() {
    let (tty, code) = run_tty_ps1(r"a\D{%Y}b", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains(r"a\D{%Y}b"), "pty: {tty:?}");
}

/// `\N`: the bash PROMPTING table has no `\N`, and bash 5.3.9 answers it as the
/// unknown-escape rule — the literal text (`a\Nb`, measured) — which is what
/// fdshell draws (D-623-3: parity, nothing to implement).
#[test]
fn an_n_in_the_prompt_is_drawn_as_its_text() {
    let (tty, code) = run_tty_ps1(r"a\Nb", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains(r"a\Nb"), "pty: {tty:?}");
}

/// A stray `\]` with no `\[` opening a region: its marker byte (0x02) is drawn as
/// the decoder's byte (bash draws the same byte, measured). `PS0` has the sibling
/// row; this is the `PS1` one.
#[test]
fn a_stray_closing_marker_on_ps1_is_drawn_as_its_byte() {
    let (tty, code) = run_tty_ps1(r"a\]b", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("a\x02b"), "pty: {tty:?}");
}

/// The three-digit octal boundary: `\12` is two digits, so it is not an octal
/// escape at all — the text stays (bash measures it literal too, measured; the
/// rule is LESSONS's three-digit rule from task #616, whose row pins the
/// three-digit case).
#[test]
fn a_two_digit_octal_escape_is_drawn_as_its_text() {
    let (tty, code) = run_tty_ps1(r"a\12b", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains(r"a\12b"), "pty: {tty:?}");
}

/// `\777`: the widest octal, truncated to a byte — 0xff reaches the pty (bash
/// measures the same byte, measured). The byte harness, because the captured stream
/// is not valid UTF-8 and the string one would panic on the decode.
#[test]
fn the_widest_octal_escape_is_drawn_truncated_to_a_byte() {
    let (tty, code) = run_tty_ps1_bytes(r"a\777b", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(
        tty.windows(b"a\xffb".len())
            .any(|window| window == b"a\xffb"),
        "the raw byte in the drawn prompt: {tty:?}"
    );
}

// ——— the value escapes (task #617), measured against bash 5.3.9 on
// 2026-11-18. The machine-dependent part of every expectation is computed here
// from the same wrappers the provider calls, and only the rules are hardcoded
// (LESSONS). ———

/// The identity escapes: `\u` from the password database, `\h` and `\H` from
/// `gethostname`, `\$` from the effective uid. bash 5.3.9 draws
/// `[damir|nixos|nixos|$] ` here (measured) — the same bytes, because this box's
/// hostname has no dot (so `\h` and `\H` agree; the split at the first dot is
/// pinned in the unit pins) and the runner is non-root.
#[test]
fn the_identity_escapes_answer_their_wrappers() {
    let user = String::from_utf8(getpwuid(geteuid()).unwrap()).unwrap();
    let host = String::from_utf8(gethostname().unwrap()).unwrap();
    let dollars = if geteuid() == 0 { "#" } else { "$" };
    let needle = format!("[{user}|{host}|{host}|{dollars}] ");
    let (tty, code, _) = run_tty(concat!(r"PS1='[\u|\h|\H|\$] '", "\nexit\n"), Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    // The typed line carries its raw backslashes, so only the prompt can match.
    assert!(tty.contains(&needle), "{needle:?} missing: {tty:?}");
}

/// `\l`: the **last component** of the tty's path — the pts number on this box,
/// which is exactly what bash draws (`[3] `, measured 2026-11-18). The number
/// differs per run, so the row pins its shape: each bracketed group is a non-empty
/// run of ASCII digits. Only the prompt is bracketed — the typed line is `exit` —
/// and the row ends with `exit`, so the group is deterministic.
#[test]
fn the_tty_escape_draws_the_last_component_of_its_path() {
    let (tty, code) = run_tty_ps1(r"[\l]", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    let drawn: Vec<&str> = tty.split(['[', ']']).skip(1).step_by(2).collect();
    assert_eq!(
        drawn.len(),
        1,
        "one drawn prompt, no bracketed echo: {tty:?}"
    );
    for group in &drawn {
        assert!(
            !group.is_empty() && group.bytes().all(|byte| byte.is_ascii_digit()),
            "the pts number's shape: {tty:?}"
        );
    }
}

/// The command counter: `\!` and `\#` both draw the number the command **about to
/// be typed** gets — `1` for the first input, `2` after one input ran, and the
/// blank line consumes neither (bash 5.3.9 measured 2026-11-18: `[263|1]`,
/// `[264|2]`, `[264|2]` — its `\!` counts the history file it loaded at startup,
/// which fdshell has no list for yet, story #21; the `\#` numbers 1, 2, 2 are the
/// rule both escapes share).
#[test]
fn the_command_counter_counts_commands_and_no_blank_line() {
    let (tty, code) = run_tty_ps1(r"[\!|\#]", "true\n\nexit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[1|1]").count(), 1, "pty: {tty:?}");
    assert_eq!(tty.matches("[2|2]").count(), 2, "pty: {tty:?}");
}

/// `\j`: the count of the shell's task map. `[0]` before the background line and
/// `[1]` for the two reads after it — the job stays in the map until `wait` reaps
/// it and nothing reaps spontaneously, so both prompts say `[1]` (bash 5.3.9
/// measured 2026-11-18 agrees on a running job: `[0]`, `[1]`, `[1]` — plus its own
/// `[1] <pid>` job line, which fdshell has none of yet). The sleep is one second,
/// so no orphan outlives the row's event.
#[test]
fn the_job_escape_counts_the_task_map() {
    let (tty, code) = run_tty_ps1(r"[\j]", "sleep 1 &\ntrue\nexit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[0]").count(), 1, "pty: {tty:?}");
    assert_eq!(tty.matches("[1]").count(), 2, "pty: {tty:?}");
}

/// The row's hand-rolled shape predicates (no regex dev-dep): the exact byte
/// pins live in the unit tests at injected instants; here the machine's own
/// clock rides into every group (LESSONS).
fn the_digits(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit())
}

fn the_hhmm(s: &str) -> bool {
    match s.split_once(':') {
        Some((a, b)) => the_digits(a, 2) && the_digits(b, 2),
        None => false,
    }
}

fn the_hhmmss(s: &str) -> bool {
    match s.split_once(':') {
        Some((a, rest)) => the_digits(a, 2) && the_hhmm(rest),
        None => false,
    }
}

fn the_letters3(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|b| b.is_ascii_alphabetic())
}

/// `%a %b %e`: three letters, space, three letters, then the space-padded day
/// as 1–2 digits (`split_whitespace` folds the pad, which is what makes the
/// single-digit-day spelling part of the shape).
fn the_date_shape(s: &str) -> bool {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match (parts.first(), parts.get(1), parts.get(2)) {
        (Some(wd), Some(mo), Some(day)) if parts.len() == 3 => {
            the_letters3(wd) && the_letters3(mo) && (the_digits(day, 1) || the_digits(day, 2))
        }
        _ => false,
    }
}

/// The 12-hour hour of `\T` and `\@`: `01..09` or `10..12`.
fn the_hour12(s: &str) -> bool {
    matches!(s, "10" | "11" | "12")
        || (s.starts_with('0')
            && matches!(
                s.get(1..2),
                Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            ))
}

fn the_ampm_shape(s: &str) -> bool {
    match s.split_once(' ') {
        Some((t, ap)) => the_hhmm(t) && matches!(ap, "AM" | "PM"),
        None => false,
    }
}

/// The clock escapes on a terminal, one row for the family (the #617 rows'
/// one-rule posture). fdshell formats in UTC at the C locale, so the exact
/// bytes depend on the machine's clock — and on bash's box, on its timezone —
/// and this row can only pin shapes: `\d` is `%a %b %e` (space-padded `%e`),
/// `\t` is 24-hour `\d\d:\d\d:\d\d`, `\T` the 12-hour spelling, `\@` is
/// `\d\d:\d\d` + space + `AM|PM` (the space is in bash's format), `\A` is
/// 24-hour `\d\d:\d\d` — all measured against bash 5.3.9 under `TZ=UTC
/// LC_ALL=C`, and pinned byte-for-byte at injected instants in
/// `value/clock/tests.rs`.
#[test]
fn the_clock_escapes_draw_their_shapes_on_a_terminal() {
    let (tty, code) = run_tty_ps1(r"[\d] [\t] [\T] [\@] [\A] ", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    let groups: Vec<&str> = tty.split(['[', ']']).skip(1).step_by(2).collect();
    assert_eq!(
        groups.len(),
        5,
        "five bracketed groups in the one prompt: {tty:?}"
    );
    assert!(the_date_shape(groups.first().unwrap()), "\\d: {tty:?}");
    assert!(the_hhmmss(groups.get(1).unwrap()), "\\t: {tty:?}");
    let (hour, rest) = groups.get(2).unwrap().split_once(':').unwrap();
    assert!(the_hour12(hour) && the_hhmm(rest), "\\T: {tty:?}");
    assert!(the_ampm_shape(groups.get(3).unwrap()), "\\@: {tty:?}");
    assert!(the_hhmm(groups.get(4).unwrap()), "\\A: {tty:?}");
}

// ——— the working-directory escapes (task #619): the display is the `%CWD`
// fd's provenance, never a re-resolution; expectations measured against bash
// 5.3.9 on 2026-11-18. Only the prompts are bracketed — the echoed commands
// and the bare `exit` carry no brackets — so each needle is the drawn prompt
// alone (the trick `the_tty_escape_draws_the_last_component_of_its_path`
// uses). ———

/// `\w` draws the origin `cd` recorded (D-619-1): `cd /tmp` stores
/// `Origin::File("/tmp")` and both answers are bash's bytes (`/tmp` is not
/// `$HOME`, so no abbreviation).
#[test]
fn the_working_directory_escape_draws_the_provenance() {
    let (tty, code) = run_tty_ps1(r"[\w][\W]", "cd /tmp\nexit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("[/tmp][tmp]"), "pty: {tty:?}");
}

/// An origin that names no path answers D2's no-value and draws the escape
/// text: the startup `open(".")` fd carries `Origin::Shell`, so the one
/// prompt of a session without a `cd` is the literal `[\w][\W]` (bash draws
/// its logical `$PWD` there — D-619-2's divergence, worded in the README's
/// prompt-time-expansion paragraph). The echoed
/// `exit` carries no backslash, so the group stays unique.
#[test]
fn an_origin_without_a_path_draws_the_escape_text() {
    let (tty, code) = run_tty_ps1(r"[\w][\W]", "exit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches(r"[\w][\W]").count(), 1, "pty: {tty:?}");
}

/// A bare `cd` records `Origin::EnvVar(HOME)`, which answers `~` — the row is
/// byte-identical to bash's measured `[~][~]` (D-619-2's parity route). `HOME`
/// is assigned in-session (the nix sandbox has none, and a store `HOME` is the
/// same lookup `cd` uses — LESSONS #282), and `.` is the test's own directory,
/// which exists on both sides.
#[test]
fn the_home_hop_draws_a_tilde() {
    let (tty, code) = run_tty_ps1(r"[\w][\W]", "HOME=.\ncd\nexit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("[~][~]"), "pty: {tty:?}");
}

/// The `unset HOME` half of D-619-4: `unset` clears the inherited `environ`
/// row as well as the store (LESSONS: "A parse-level keyword inherits an
/// operand grammar — check the grammar you inherit"), so the shell's `HOME`
/// lookup answers `None` and the abbreviation is dropped — byte-identical to
/// bash 5.3.9 measured on 2026-11-18: `[~][~]` while `HOME` is `/tmp`,
/// `[/tmp][tmp]` after the `unset`. The unit row
/// `value/cwd/tests.rs::an_absent_home_drops_the_abbreviation` is the other
/// half. `HOME=/tmp` is assigned in-session so the row is deterministic on a
/// sandbox with no `HOME`, and `/tmp` exists on both sides.
#[test]
fn unset_home_drops_the_abbreviation() {
    let (tty, code) = run_tty_ps1(r"[\w][\W]", "HOME=/tmp\ncd /tmp\nunset HOME\nexit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[~][~]").count(), 1, "pty: {tty:?}");
    assert_eq!(tty.matches("[/tmp][tmp]").count(), 1, "pty: {tty:?}");
}

/// `PROMPT_DIRTRIM` on `/proc/1` (two components): the plain rule trims at
/// `components > n`, so n=1 **does** trim — the measured row is `[.../1][1]`,
/// bash-identical (a one-component `/tmp` here cannot trim: count == n,
/// measured no-trim both shells).
#[test]
fn dirtrim_ellipsis_replaces_the_leading_components() {
    let (tty, code) = run_tty_ps1(r"[\w][\W]", "cd /proc/1\nPROMPT_DIRTRIM=1\nexit\n");
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("[.../1][1]"), "pty: {tty:?}");
}

// ——— prompt-time expansion (task #620), measured against bash 5.3.9 on
// 2026-11-18, `promptvars` on by default (`tests/shopt.rs` pins the shopt). Every
// stored `PS1` is quoted, so its triggers are prompt-time, and every needle is a
// byte stream the pty echo cannot produce — the typed line keeps the raw `$v` or
// `$( … )` text — except where a count says otherwise. The rows are one per fact:
// every `$( … )` and backtick in a prompt forks, so a prompt with one pays a fork
// per prompt drawn (the resident-decorator answer is epic #199). ———

/// The two name forms, on both prompts: `PS1` at the `exit` read, and `PS2` at
/// the buffered one (bash draws `V `, `V ` and — measured with `IFS=,` and two
/// positionals — `[V|a,b|a b] `).
#[test]
fn a_prompt_expands_its_variables() {
    let (tty, code, _) = run_tty("v=V\nPS1='[$v|${v}] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[V|V] ").count(), 1, "pty: {tty:?}");

    let (tty, code, _) = run_tty("v=V\nPS2='[$v] '\nif true; then\nfi\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[V] ").count(), 1, "pty: {tty:?}");
    assert_eq!(tty.matches(PROMPT).count(), 4, "pty: {tty:?}");
}

/// The three substitution arms in one prompt: command substitution, backticks and
/// arithmetic (bash draws `hi `, `bt ` and `2 ` on the same stored shapes).
#[test]
fn a_prompt_fires_the_command_and_arithmetic_arms() {
    let (tty, code, _) = run_tty(
        "PS1='[a$(echo one)b`echo two`c$((1+1))d] '\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[aonebtwoc2d] ").count(), 1, "pty: {tty:?}");
}

/// `shopt -u promptvars` keeps the decoder's work and skips the expansion — and
/// quote removal is part of the skipped expansion (measured). Both ways in one
/// row: the prompt drawn before the `shopt -u` read is the expanded one.
#[test]
fn promptvars_off_skips_the_expansion() {
    let (tty, code, _) = run_tty(
        "PS1='[a$((1+1))b] '\nshopt -u promptvars\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[a2b] ").count(), 1, "pty: {tty:?}");
    assert_eq!(
        tty.matches("[a$((1+1))b] ").count(),
        2,
        "echo + prompt: {tty:?}"
    );
}

/// The nounset row (D-620-8): bash reports on stderr, draws the prompt as the
/// unexpanded text and keeps prompting. fdshell's wording is its own (the README's
/// prompt-time-expansion paragraph words it); the shell stays alive on bash's input,
/// and the report names the variable bash names.
#[test]
fn nounset_in_a_prompt_reports_and_keeps_the_shell() {
    let (tty, code, _) = run_tty("set -u\nPS1='[a$nope] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "the REPL must keep prompting: {tty:?}");
    assert!(tty.contains("nope: unbound variable"), "pty: {tty:?}");
    assert_eq!(
        tty.matches("[a$nope] ").count(),
        2,
        "echo + prompt: {tty:?}"
    );
}

/// The `%fd` arm is live on this surface (D-620-5): a bound name draws its fd
/// number, where bash draws `%CWD` literally — fdshell's own answer, pinned by
/// its shape, since the number is the shell's own bookkeeping.
#[test]
fn the_prompt_fd_arm_fires() {
    let (tty, code, _) = run_tty("PS1='[a%CWD] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    let groups: Vec<&str> = tty.split(['[', ']']).skip(1).step_by(2).collect();
    assert_eq!(groups.len(), 2, "the echo and the drawn prompt: {tty:?}");
    assert_eq!(
        groups.first().copied(),
        Some("a%CWD"),
        "the typed line: {tty:?}"
    );
    let drawn = groups.get(1).unwrap_or(&"");
    assert!(
        drawn
            .strip_prefix('a')
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())),
        "the drawn prompt names the fd's number: {tty:?}"
    );
}

/// `set -- a b` + `IFS=,` + `[${*}|${@}] ` → `[a,b|a b] ` (measured): `$*` joins
/// with the first `IFS` byte, `$@` **space**-joins — the join mode the prompt's
/// entry point names (D-620-6, LESSONS: "`$@` has two join rules").
#[test]
fn the_prompt_positional_join_is_the_measured_one() {
    let (tty, code, _) = run_tty("set -- a b\nIFS=,\nPS1='[${*}|${@}] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[a,b|a b] ").count(), 1, "pty: {tty:?}");
}

/// Nothing else expands on this surface (all measured): no tilde — the runner's
/// `HOME` is set, so a live arm would print it — no globbing, no field splitting,
/// and the separator bytes are kept. The row counts the typed shape because the
/// drawn prompt is byte-identical to it.
#[test]
fn nothing_else_expands_on_the_prompt_surface() {
    let (tty, code, _) = run_tty("PS1='[~|*|a  b|a;b] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(
        tty.matches("[~|*|a  b|a;b] ").count(),
        2,
        "echo + prompt: {tty:?}"
    );
}

/// Quote characters are literal data **and** they do not protect (measured
/// `"$v" ` → `"V" `): the quotes survive and the expansion inside them fires.
#[test]
fn quote_characters_in_a_prompt_are_data() {
    let (tty, code, _) = run_tty("v=V\nPS1='[\"$v\"] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[\"V\"] ").count(), 1, "pty: {tty:?}");
}

/// The backslash set, end to end (measured): `\` before `$ \`" ' \ is a quote
/// operator and loses its own byte; before anything else it survives. The `\\`
/// pair is the decoder's mechanical escape, so its drawn byte is one backslash —
/// and `\q` stays, which is what separates the two arms of the escape grammar.
#[test]
fn the_prompt_backslash_quotes_only_the_special_set() {
    let (tty, code, _) = run_tty(
        concat!(r#"PS1='[a\"b|a\\b|\\qX] '"#, "\nexit\n"),
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[a\"b|a\\b|\\qX] ").count(), 1, "pty: {tty:?}");
}

/// The `\xHH` divergence end to end (decision D1 of task #616: bash measures
/// `\x` literal, fdshell decodes it): a byte the decoder produced is part of the
/// run and **is** scanned, so fdshell draws `[V] ` where bash draws `\x24v `.
#[test]
fn a_hex_decoded_dollar_expands_in_a_prompt() {
    let (tty, code, _) = run_tty("v=V\nPS1='[\\x24v] '\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("[V] ").count(), 1, "pty: {tty:?}");
}

/// A capture-limit failure in the prompt path follows D-620-8: the cap
/// (`MAX_CAPTURED`, lowered by `set --stdout-capture-limit`) applies to a prompt's
/// `$( … )` exactly as it does anywhere else, but the REPL reports and keeps
/// prompting — it must not die because a prompt grew past the cap — and the prompt
/// is drawn as the unexpanded text.
#[test]
fn an_over_cap_command_substitution_keeps_the_shell() {
    let (tty, code, _) = run_tty(
        "set --stdout-capture-limit 4\nPS1='[a$(echo toolongword)b] '\ntrue\nexit\n",
        Event::Exit,
    );
    // Kept prompting: the report and the unexpanded prompt come out once per
    // prompt drawn, twice. The exit code is the last status, as `exit` takes it
    // with no argument: the over-cap producer is SIGKILLed (128 + 9).
    assert_eq!(code, Some(137), "pty: {tty:?}");
    assert_eq!(tty.matches("capture limit").count(), 2, "pty: {tty:?}");
    assert_eq!(
        tty.matches("[a$(echo toolongword)b] ").count(),
        3,
        "echo + two prompts: {tty:?}"
    );
}

/// The gate itself over a pipe (bash draws no prompt off-terminal, measured): a
/// set `PS1`/`PS2` produce no prompt bytes at all, so stdout is the body alone.
/// The pty rows above are the other half of the gate.
#[test]
fn no_prompt_off_a_terminal() {
    let (out, err, code) = run_repl("PS1='[p1] '\necho x\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "x\n", "no prompt bytes on a pipe: {out:?}");
    assert!(!out.contains(PROMPT), "stdout: {out:?}");

    let (out, err, code) = run_repl("PS2='[p2] '\nif true; then\nfi\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert!(out.is_empty(), "no prompt bytes on a pipe: {out:?}");
    assert!(!out.contains("[p2] "), "stdout: {out:?}");
}

// ——— `PS0` (task #622), the value bash ≥ 5.1 **displays** after reading a complete
// input line and before its command runs. bash's `NEWS` entry says "executed", and
// the measured behavior, `man bash` and bash's `eval.c` all say displayed; the rows
// below are the measurements (bash 5.3.9, 2026-11-20) through the pty harness of
// LESSONS: "Test tty-gated behavior from safe-crate integration tests via `script`".
// Every row ends with `exit` (the exit is the bounded event) and every needle is a
// byte stream the pty echo cannot produce — the typed line keeps the raw `$v`, the
// raw `\#`, the raw `\[` — except where a count says otherwise. The counts are
// displays **including** the one for the final `exit`, and never one for the line
// that sets `PS0` while it is still empty. ———

/// The "PS0 running an echo" row: the display lands between the read and the
/// command's own output, with **no trailing newline** — `p-Vout` is one display and
/// the `echo`'s output, concatenated. Three displays: the reads of `true`, of
/// `echo out` and of `exit`; the assignment's own read shows nothing, because the
/// value shown is the one held when the line was read, and `PS0` was empty then
/// (measured on both sides).
#[test]
fn ps0_shows_between_the_read_line_and_the_command() {
    let (tty, code, _) = run_tty("v=V\nPS0='p-$v'\ntrue\necho out\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 3, "pty: {tty:?}");
    assert!(
        tty.contains("p-Vout"),
        "no newline between the display and the output: {tty:?}"
    );
}

/// `shopt -u promptvars` gates `PS0`'s expansion exactly as it gates `PS1`/`PS2`
/// and `PS4`: the two reads before the `shopt` line show the expanded value, the two
/// after it show the stored text as it stands. The raw-text count is those two
/// displays plus the pty's echo of the typed assignment line (measured: bash draws
/// `p-V`, `p-V`, `p-$v`, `p-$v` — the row's three raw matches are two displays and
/// the echo).
#[test]
fn promptvars_off_shows_ps0_unexpanded() {
    let (tty, code, _) = run_tty(
        "v=V\nPS0='p-$v'\ntrue\nshopt -u promptvars\ntrue\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 2, "pty: {tty:?}");
    assert_eq!(
        tty.matches("p-$v").count(),
        3,
        "echo + two displays: {tty:?}"
    );
}

/// The one prompt variable with **no** default string: unset and set-empty both
/// answer nothing, so the default `PS0` is a no-op. First row: two displays — the
/// read of `true` and the read of the `unset` itself, which still shows the value
/// that is about to go — and silence for the two reads after it (measured: bash shows
/// the same two). Second row: the value emptied mid-row shows exactly once, for the
/// read of the line that empties it, and never again (measured, both sides).
#[test]
fn ps0_unset_and_set_empty_show_nothing() {
    let (tty, code, _) = run_tty(
        "v=V\nPS0='p-$v'\ntrue\nunset PS0\ntrue\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 2, "pty: {tty:?}");

    let (tty, code, _) = run_tty("v=V\nPS0='p-$v'\nPS0=''\ntrue\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 1, "pty: {tty:?}");
}

/// The row that decides display-not-execute, because a real execution would fail it:
/// a `PS0` whose value *is* a command leaves `$?` alone — `true` then `rc=0`, `false`
/// then `rc=1`, both measured against bash 5.3.9 (which prints the text `echo` and
/// its argument rather than running them).
#[test]
fn the_exit_status_survives_ps0() {
    let (tty, code, _) = run_tty("v=V\nPS0='p-$v'\ntrue\necho rc=$?\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 3, "pty: {tty:?}");
    assert_eq!(tty.matches("rc=0").count(), 1, "pty: {tty:?}");

    let (tty, code, _) = run_tty("v=V\nPS0='p-$v'\nfalse\necho rc=$?\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 3, "pty: {tty:?}");
    assert_eq!(tty.matches("rc=1").count(), 1, "pty: {tty:?}");
}

/// One fork per use, the rule the prompt expansion already pays for `PS1`: the
/// substitution re-runs at every display, so the needle is the substitution's answer
/// three times — bash draws three too (measured). Where they part is `$?`: bash
/// preserves the command's own status (measured `rc=1`), while here the fork's status
/// lands in it, the expansion stage's pre-existing rule that `PS1` follows too.
#[test]
fn a_command_substitution_in_ps0_runs_per_use() {
    let (tty, code, _) = run_tty(
        "PS0='p$(true; echo sub)'\nfalse\necho rc=$?\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("psub").count(), 3, "pty: {tty:?}");
    assert!(
        tty.contains("psubrc=0"),
        "bash preserves the status and measures rc=1: {tty:?}"
    );
}

/// One display for a **whole construct**, not one per line: the `if` block is one
/// input, so the two displays measured on both sides are its read and the `exit`
/// read — and the construct's own output follows its display with no newline
/// between, the concatenation pinned here.
#[test]
fn ps0_shows_once_for_a_whole_construct() {
    let (tty, code, _) = run_tty(
        "v=V\nPS0='p-$v'\nif true; then\n echo INBLOCK\nfi\nexit\n",
        Event::Exit,
    );
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 2, "pty: {tty:?}");
    assert!(
        tty.contains("p-VINBLOCK"),
        "the display precedes the construct it accompanies: {tty:?}"
    );
}

/// The number `PS0` shows is the number of the command it accompanies, so the display
/// runs **before** the counter moves: `p-2` for `true` and `p-3` for `exit`, because
/// the assignment was command 1 — its own read is silent, `PS0` was empty for it —
/// and the provider answers the number the next command gets (measured against bash,
/// which draws the same two numbers). A display placed after the increment would draw
/// `p-3` and `p-4`.
#[test]
fn ps0_shows_the_number_of_the_command_it_accompanies() {
    let (tty, code, _) = run_tty("PS0='p-\\#'\ntrue\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-2").count(), 1, "pty: {tty:?}");
    assert_eq!(tty.matches("p-3").count(), 1, "pty: {tty:?}");
}

/// A marker that does **not** toggle the region keeps readline's byte — `PS1`'s
/// policy, byte-identical to bash's measured display (`a\]b` → `a\002b`); the
/// in-process mirror is `src/repl/prompt/ps0/tests.rs::a_stray_marker_keeps_its_readline_byte`.
#[test]
fn a_stray_marker_keeps_its_readline_byte_in_ps0() {
    let (tty, code, _) = run_tty("PS0='a\\]b'\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("a\x02b").count(), 1, "pty: {tty:?}");
}

/// The arm the policy does not reach: a marker that **does** toggle is a record split
/// and is consumed, so fdshell draws the region's bytes (`ABC`), where bash's display
/// — a raw write of the decoded string — prints `A\001B\002C`. fdshell's own answer,
/// the same fact `a_non_printing_region_is_drawn_and_recorded` pins one drawer up, and
/// the README's prompt-time-expansion paragraph words it.
#[test]
fn a_toggling_marker_contributes_no_bytes_in_ps0() {
    let (tty, code, _) = run_tty("PS0='A\\[B\\]C'\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(
        tty.matches("ABC").count(),
        1,
        "bash prints A\\001B\\002C here: {tty:?}"
    );
}

/// `PS0` is displayed on the stream bash uses — stderr (measured). With the shell's
/// stdout diverted to a file, the three displays are in the pty dialogue and the file
/// holds the prompts and the command's own output but no display bytes. The `env` form
/// is because the value has to be in place before the first prompt (the reason
/// [`an_inherited_env_value_reaches_the_prompt`] spells itself the same way).
#[test]
fn ps0_is_displayed_on_the_stream_bash_uses() {
    let path = std::env::temp_dir().join(format!("fdshell-ps0-stream-{}", std::process::id()));
    let mut child = Command::new("script")
        .args([
            "-qec",
            &format!("env PS0='p0txt' {BIN} >{}", path.display()),
            "/dev/null",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"v=V\necho tofile\nexit\n")
        .unwrap();
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    let dialogue = str::from_utf8(&output.stdout).unwrap().to_string();
    assert_eq!(output.status.code(), Some(0), "pty: {dialogue:?}");
    assert_eq!(dialogue.matches("p0txt").count(), 3, "pty: {dialogue:?}");
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(
        !file.contains("p0txt"),
        "the diverted stdout holds no display: {file:?}"
    );
    assert!(
        file.contains("tofile"),
        "the command's own output: {file:?}"
    );
    std::fs::remove_file(&path).unwrap();
}

/// The gate over a pipe (bash shows nothing off a terminal, measured): the pty rows
/// above are the interactive half. `echo` is the only thing written, on either stream.
#[test]
fn no_ps0_off_a_terminal() {
    let (out, err, code) = run_repl("v=V\nPS0='p-$v'\necho x\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "x\n", "no display bytes off a terminal: {out:?}");
    assert!(err.is_empty(), "stderr: {err:?}");
}

/// The `set -x` ordering the task asks for, per command: the read, then the display,
/// then the trace line — `p-V+ true`, the concatenation bash produces too (measured),
/// because both the display and the trace happen at execution.
#[test]
fn ps0_shows_before_what_the_command_itself_prints() {
    let (tty, code, _) = run_tty("v=V\nPS0='p-$v'\nset -x\ntrue\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("p-V+ true"), "bash measures the same: {tty:?}");
}

/// fdshell's own answer where it diverges from bash, pinned for the wording the
/// README's prompt-time-expansion paragraph carries: bash
/// reports a parse error with **no** display (measured one `p-V`, for the `exit`
/// read), while fdshell displays the value and then reports — the display belongs to
/// the read, the report to the parse, and the read comes first.
#[test]
fn ps0_shows_before_a_parse_error_is_reported() {
    let (tty, code, _) = run_tty("v=V\nPS0='p-$v'\nfi\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(tty.matches("p-V").count(), 2, "bash shows one: {tty:?}");
    let display = tty.find("p-V").unwrap();
    let report = tty.find("parse error").unwrap();
    assert!(display < report, "the display, then the report: {tty:?}");
}

/// The other divergence, the one `src/verbose.rs` already records: bash echoes at
/// read time, so its echo precedes the display (measured `true`, then `p-V`), while
/// fdshell echoes at execution time and the display comes first — `p-Vtrue`, no
/// newline between.
#[test]
fn ps0_shows_before_the_verbose_echo() {
    let (tty, code, _) = run_tty("v=V\nPS0='p-$v'\nset -v\ntrue\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(
        tty.contains("p-Vtrue"),
        "bash measures true-then-p-V here: {tty:?}"
    );
}

/// The plain-prompt shape of `set -v` (bash 5.3.9 measured 2026-11-20: the
/// `set -v` line itself gets no echo, and the echo of the `true` read lands between
/// the prompt that read it and the next one — the same window fdshell's execution-time
/// echo answers it in, so the plain shape agrees; the read-time/exec-time split shows
/// through `PS0`, and `ps0_shows_before_the_verbose_echo` pins that).
/// `{PROMPT}{PROMPT}true` is fdshell's byte shape: no line echo of its own, so the
/// two prompts sit back to back and the echo follows its prompt.
#[test]
fn verbose_echoes_after_the_prompt_that_read_its_line() {
    let (tty, code, _) = run_tty("set -v\ntrue\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(
        tty.contains(&format!("{PROMPT}{PROMPT}true")),
        "the echo comes after its own prompt: {tty:?}"
    );
}

/// The `set -x` line itself: fdshell traces it (`+ set -x`, then `+ true`), while
/// bash 5.3.9 does **not** trace the line that turns tracing on (measured: prompt,
/// then `+ true` alone). The second trace is the parity half of the row.
#[test]
fn the_set_x_line_is_traced_though_bash_does_not_trace_it() {
    let (tty, code, _) = run_tty("set -x\ntrue\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert!(tty.contains("+ set -x"), "bash measures none: {tty:?}");
    assert!(tty.contains("+ true"), "bash measures the same: {tty:?}");
}

/// A multi-line command under `set -x` (bash 5.3.9 measured 2026-11-20: both `PS2`
/// prompts as it reads the `if`, then the two traces at execution — `+ true`,
/// `+ true`). fdshell keeps the agreement: both continuation prompts, then the two
/// traces; the bytes run together only because fdshell has no line editor to echo
/// the typed lines (the README's prompt-layer divergence paragraph owns that).
#[test]
fn a_multi_line_command_prompts_then_traces() {
    let (tty, code, _) = run_tty("set -x\nif true; then\ntrue\nfi\nexit\n", Event::Exit);
    assert_eq!(code, Some(0), "pty: {tty:?}");
    assert_eq!(
        tty.matches("+ true").count(),
        2,
        "bash traces twice too: {tty:?}"
    );
    assert!(
        tty.contains(&format!("{PROMPT}{CONT}{CONT}+ true")),
        "both prompts, then the first trace: {tty:?}"
    );
}

/// An inherited `PS0` reaches the display through the same shared lookup as `PS1` —
/// the `env` form of [`an_inherited_env_value_reaches_the_prompt`], for the same
/// reason — and it is expanded on the way: the value `p-$v` draws `p-` for the read
/// of `v=V` (the variable is still unset) and `p-V` for the read of `exit`. bash
/// measures the same two (measured 2026-11-20).
#[test]
fn an_inherited_env_value_reaches_ps0() {
    let mut child = Command::new("script")
        .args(["-qec", &format!("env PS0='p-$v' {BIN}"), "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"v=V\nexit\n")
        .unwrap();
    drop(child.stdin.take().unwrap());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let out = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(out.matches("p-V").count(), 1, "pty: {out:?}");
    assert_eq!(
        out.matches("p-").count(),
        2,
        "the unset read and this one: {out:?}"
    );
}

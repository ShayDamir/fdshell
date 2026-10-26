#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// The prompt printed before every read. Pinned byte-for-byte: it is the
/// counter the blank-line tests below use, so it pins one prompt per read.
const PROMPT: &str = "fdshell> ";

/// The `ignoreeof` hint printed at an ignored EOF (task #291).
const HINT: &str = "use `exit' to leave";

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

/// Spawn the REPL under a pty (`script -qec <bin>`, so `isatty(0)` is true),
/// write `input`, close the pty's stdin (that does deliver a pty EOF), settle,
/// and report stdout plus whether the shell was still alive.
///
/// No stdout read loop: after the `ignoreeof` hint a pty with closed stdin
/// delivers nothing, so a blocking `read` hangs to nextest's 10 s
/// slow-timeout. Sleep → `try_wait` → `kill` → drain is the shape that bounds
/// the output. The pty echoes the input and inserts `\r\n`, so assertions
/// `contains` and count matches, never exact stdout equality.
fn run_tty(input: &str) -> (String, bool) {
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
    std::thread::sleep(std::time::Duration::from_millis(500));
    let alive = child.try_wait().unwrap().is_none();
    if alive {
        let _ = child.kill();
    }
    let output = child.wait_with_output().unwrap();
    (str::from_utf8(&output.stdout).unwrap().to_string(), alive)
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

/// Task #322: a here-doc inside a **closed** single-line keyword block keeps
/// the prompt reading for its body lines (`> ` ×2) and the buffer runs with
/// the body attached — bash 5.3.9 prints `{a,b}`, rc 0 on every row; the
/// prompt trail is two `fdshell> ` reads (the shape, then `exit`) around the
/// two `> ` continuations.
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
        assert_eq!(
            out, "fdshell> > > {a,b}\nfdshell> ",
            "{row}: stdout: {out:?}"
        );
        assert!(err.is_empty(), "{row}: stderr={err}");
    }
}

/// Task **#332**: a here-doc statement **before** a `{ …; }` group and a
/// statement after the group's own `;`. The line carries two body regions, so the
/// prompt reads four `> ` continuations — bash's continuation count on the same
/// stdin — and the buffer runs as `bodyX`, `a`, `bodyB` at rc 0.
///
/// `master` stopped after two `> ` reads (the line looked complete with region 0
/// consumed) and then reported the body lines of region 1 as missing commands,
/// `"bodyX" not found` … ; with the group's resume fixed the trailing statement
/// is scanned where its own region lives. Measured byte-for-byte.
#[test]
fn pre_block_heredoc_behind_a_group_spans_both_regions() {
    let (out, err, code) = run_repl("cat <<X; { echo a; }; cat <<B\nbodyX\nX\nbodyB\nB\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(
        out, "fdshell> > > > > bodyX\na\nbodyB\nfdshell> ",
        "stdout: {out:?}"
    );
    assert!(err.is_empty(), "stderr={err}");
}

/// Task #322, pinned divergence of the same fix at the prompt.
///
/// The `elif` row at EOF: the `E` delimiter never arrives, the buffer executes
/// and reports `missing terminating delimiter line 'E'`, stdout is prompts only,
/// rc 0 — the incomplete-body-stream rule pinned in script mode by
/// `tests/heredoc.rs::cond_bodies_of_an_incomplete_line_stream_are_empty`; bash
/// 5.3.9 prints `{a,b}\nT` with an EOF-delimiter warning, rc 0. The group twin
/// `{ cat <<T; }` flipped to bash parity with task **#315** and is pinned in the
/// family above.
#[test]
fn closed_block_heredoc_prompt_divergences() {
    // EOF (no `exit`): the third `> ` consumes EOF, the buffer executes.
    let (out, err, code) = run_repl("if false; then :; elif true; then cat <<E; fi\n{a,b}\nT\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "fdshell> > > > fdshell> ", "stdout: {out:?}");
    assert!(
        err.contains("missing terminating delimiter line 'E'"),
        "stderr={err}"
    );
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
    assert_eq!(out.matches(PROMPT).count(), 3, "prompts: {out:?}");
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

/// A blank line is not an input: the shell re-prompts and keeps reading, so
/// only end of input (`exit`, EOF) ends it. Prompt counts are read counts —
/// line, blank line, `exit` — and a shell that quit on the blank line prints
/// two.
#[test]
fn blank_line_does_not_end_the_shell() {
    let (out, err, code) = run_repl("echo hi\n\nexit\n");
    assert_eq!(code, 0, "stderr: {err:?}");
    assert!(out.contains("hi"), "output: {out:?}");
    assert_eq!(out.matches(PROMPT).count(), 3, "prompts: {out:?}");
}

/// Whitespace-only input is blank too: it is skipped with a prompt and never
/// dispatched (dispatching `   ` reports a parse error on stderr).
#[test]
fn whitespace_line_is_skipped_not_executed() {
    let (out, err, code) = run_repl("   \necho hi\nexit\n");
    assert_eq!(code, 0, "stderr: {err:?}");
    assert!(out.contains("hi"), "output: {out:?}");
    assert!(!err.contains("parse error"), "stderr: {err:?}");
    assert_eq!(out.matches(PROMPT).count(), 3, "prompts: {out:?}");
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

/// `ignoreeof` on a terminal, EOF mid-continuation: the terminal delivers one
/// EOF event, the hint prints and the shell stays alive at the `> ` prompt
/// (measured: `fdshell> > use \`exit' to leave\r\n> `, then blocked).
#[test]
fn ignoreeof_mid_continuation_keeps_shell_alive_on_terminal() {
    let (out, alive) = run_tty("if true; then\n");
    assert!(alive, "shell died at tty EOF mid-continuation: {out:?}");
    assert_eq!(
        out.matches(HINT).count(),
        1,
        "ignoreeof hint count: {out:?}"
    );
    assert!(
        out.replace(PROMPT, "").contains("> "),
        "continuation prompt missing: {out:?}"
    );
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
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr={err}");
        assert!(
            out.contains("> "),
            "{input}: no continuation prompt: {out:?}"
        );
        assert_eq!(out.matches(PROMPT).count(), 3, "prompts: {out:?}");
        let body = out.replace(PROMPT, "").replace("> ", "");
        assert_eq!(body, want, "{input}: stdout: {out:?}");
        assert!(
            !err.contains("parse error"),
            "{input}: parse error: {err:?}"
        );
        assert!(
            want_err.is_empty() || err.contains(want_err),
            "{input}: stderr={err:?}"
        );
    }
    // EOF mid-buffer: the buffer is executed and reports the parse error —
    // master's infinite loop (`while :; do echo x done` was accepted) is a
    // regression here, so the row is pinned with a closed stdin.
    for input in ["while :; do echo x done\n", "if a; then b fi\n"] {
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr={err}");
        assert_eq!(out.matches(PROMPT).count(), 2, "prompts: {out:?}");
        assert!(
            out.replace(PROMPT, "").contains("> "),
            "{input}: no continuation prompt: {out:?}"
        );
        assert!(
            err.contains("parse error"),
            "{input}: the buffer was not executed: {err:?}"
        );
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
    let (out, err, code) = run_repl("for i in done\ndo echo \"$i\"; done\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.matches(PROMPT).count(), 2, "prompts: {out:?}");
    assert!(
        out.contains("> "),
        "the first line was dispatched, not buffered: {out:?}"
    );
    assert_eq!(
        out.replace(PROMPT, "").replace("> ", ""),
        "done\n",
        "the loop iterates over the list word: {out:?}"
    );
    assert!(!err.contains("parse error"), "parse error: {err:?}");
    // EOF mid-buffer: the buffer is executed and reports the missing closer.
    let (out, err, code) = run_repl("for i in done\nexit\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.matches(PROMPT).count(), 2, "prompts: {out:?}");
    assert!(
        out.replace(PROMPT, "").contains("> "),
        "no continuation prompt: {out:?}"
    );
    assert!(
        err.contains("'for' is missing its closing 'done'"),
        "the buffer was not executed as an unclosed block: {err:?}"
    );
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
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr={err}");
        assert!(err.contains(want), "{input}: stderr={err}");
        assert_eq!(
            err.matches(want).count(),
            1,
            "{input}: the diagnostic is reported once: {err}"
        );
        // One primary prompt per read (the shape, then the read after the
        // EOF-executed buffer) and exactly one `> ` continuation line.
        assert_eq!(out.matches(PROMPT).count(), 2, "prompts: {out:?}");
        assert_eq!(
            out.replace(PROMPT, "").matches("> ").count(),
            1,
            "{input}: continuations: {out:?}"
        );
    }
}

/// A **closed** multi-line mid-run block is buffered and runs as **one**
/// statement, the bash parity row of task #294: `true && while :; do` +
/// `echo hi` + `break` + `done` prints `hi` (the `break` is inside a loop, so
/// the lines never run apart), and the group twin prints its two lines.
#[test]
fn closed_midrun_construct_runs_as_one_statement() {
    let (out, err, code) = run_repl("true && while :; do\necho hi\nbreak\ndone\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert!(
        !err.contains("not inside a loop"),
        "the lines ran apart: {err}"
    );
    assert!(!err.contains("parse error"), "parse error: {err}");
    // The buffer's own output, prompts stripped: just `hi`.
    let body = out.replace(PROMPT, "").replace("> ", "");
    assert_eq!(body, "hi\n", "stdout: {out:?}");
    assert_eq!(out.matches(PROMPT).count(), 2, "prompts: {out:?}");
    assert_eq!(
        out.replace(PROMPT, "").matches("> ").count(),
        3,
        "continuations: {out:?}"
    );

    let (out, err, code) = run_repl("true && { echo g\necho h\n}\necho tail\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert!(!err.contains("parse error"), "parse error: {err}");
    let body = out.replace(PROMPT, "").replace("> ", "");
    assert_eq!(body, "g\nh\ntail\n", "stdout: {out:?}");
    assert_eq!(out.matches(PROMPT).count(), 3, "prompts: {out:?}");
    assert_eq!(
        out.replace(PROMPT, "").matches("> ").count(),
        2,
        "continuations: {out:?}"
    );
}

/// A closed **single-line** mid-run construct never prompts: the stamp is the
/// span's closedness, so a line whose closer is there runs at once — the
/// continuation prompt is the unclosed shape's alone (task #294). Every loop
/// row is bounded by a `break` (a `while :; do :; done` twin would loop
/// forever in the REPL as it does in bash, and hang the suite).
#[test]
fn closed_single_line_midrun_never_prompts() {
    for (input, want) in [
        ("true && { echo g; }\nexit\n", "g"),
        ("true && while :; do echo w; break; done\nexit\n", "w"),
        // A definition writes no output of its own, so the call that follows
        // shows the mid-run `f() { … }` did define it.
        ("true && f() { echo F; }\nf\nexit\n", "F"),
    ] {
        let (out, err, code) = run_repl(input);
        assert_eq!(code, 0, "stderr: {err}");
        assert!(!err.contains("parse error"), "parse error: {err}");
        assert!(out.contains(want), "{input}: stdout: {out:?}");
        assert!(
            !out.replace(PROMPT, "").contains("> "),
            "{input}: continuation prompt: {out:?}"
        );
    }
    // The closed **stage** arm: its parse-plus-output oddity is #292's (the rc
    // and the `expected 'done'` family), so only the continuation prompt is
    // pinned here — the shape never enters the buffer, and the stage runs.
    let (out, _, _) = run_repl("echo a | while read l; do echo \"$l\"; done\nexit\n");
    assert!(out.contains('a'), "stdout: {out:?}");
    assert!(
        !out.replace(PROMPT, "").contains("> "),
        "continuation prompt: {out:?}"
    );
}

/// The documented divergence at EOF (README, "Interactive shell", task #294):
/// fdshell executes the buffer it has, so the statements written **before** an
/// unclosed mid-run construct run (`echo a` prints) and the construct then
/// reports its missing closer — where bash parses the whole line first, runs
/// nothing, and exits 2.
#[test]
fn statements_before_an_unclosed_midrun_run_at_eof() {
    let (out, err, code) = run_repl("echo a; true && { echo g\n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.matches(PROMPT).count(), 2, "prompts: {out:?}");
    assert_eq!(
        out.replace(PROMPT, "").matches("> ").count(),
        1,
        "continuations: {out:?}"
    );
    assert!(out.contains('a'), "the prefix statement ran: {out:?}");
    assert!(
        err.contains("group is missing a closing brace"),
        "stderr={err}"
    );
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
    assert!(out.contains("> "), "no continuation prompt: {out:?}");
    // Three prompts: the shape, the joined buffer, `exit` — one per read.
    assert_eq!(out.matches(PROMPT).count(), 3, "prompts: {out:?}");
    assert!(out.contains("a"), "stdout: {out:?}");
    assert!(!err.contains("parse error"), "parse error: {err}");
}

/// A here-doc statement **behind** a plain `&` on the same line (task #313):
/// the prompt keeps reading the four body lines (`> ` ×4 — both bodies), the
/// buffer runs with both bodies attached, and no body line is ever treated as
/// a statement (before the fix the suffix's body lines answered as `y` and `B`
/// "not found"). The background job is reaped by the `wait` line, and the
/// prompt count is one per read: the shape, `wait`, `exit`.
#[test]
fn heredocs_on_both_sides_of_an_ampersand_span_their_lines() {
    let (out, err, code) = run_repl("cat <<A & cat <<B\nx\nA\ny\nB\nwait\nexit\n");
    assert_eq!(code, 0, "stderr: {err}");
    assert!(
        out.starts_with("fdshell> > > > > "),
        "four continuations expected: {out:?}"
    );
    assert_eq!(out.matches(PROMPT).count(), 3, "prompts: {out:?}");
    // The continuation prompts, counted outside the `fdshell> ` prompts (each
    // of which ends in `> `).
    assert_eq!(
        out.replace(PROMPT, "").matches("> ").count(),
        4,
        "continuations: {out:?}"
    );
    // The bodies are the output; their order is the job's, so never pinned.
    assert!(out.contains('x') && out.contains('y'), "stdout: {out:?}");
    assert!(
        !err.contains("not found"),
        "body line ran as a statement: {err}"
    );
    assert!(!err.contains("parse error"), "parse error: {err}");
}

#![allow(clippy::unwrap_used)]

//! Empty block bodies (task #233, and the function-definition body of task
//! #302): POSIX #2.9 requires a non-empty `list` after `then`, `elif … then`,
//! `else`, `do` and inside a function definition's `{ … }`; bash 5.3.9 rejects
//! every empty form with rc 2 (measured 2026-10-12, the function rows
//! 2026-10-23), fdshell reports its own message with rc 1 (README "Errors").
//! The empty-`do` forms used to **run forever** (`while true; do; done` is an
//! unbounded no-op loop), so the two loop pins below run under a bounded
//! child: a regression must panic, not hang.

use std::process::{Command, Stdio};
use std::str;
use std::time::{Duration, Instant};
use sys::Pid;
use sys::signal::{SIGKILL, kill};

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

/// 5 s, strictly under nextest's 10 s `slow-timeout` (`.config/nextest.toml`),
/// so the panic below — not a bare timeout — reports a regressed hang.
const BOUND: Duration = Duration::from_secs(5);

/// All pids whose parent is `parent` (the same helper as `tests/loops.rs`).
fn children_of(parent: u32) -> Vec<u32> {
    let mut pids = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap() {
        let name = entry.unwrap().file_name();
        let Ok(pid) = name.to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        let fields = stat.rsplit(')').next().unwrap_or("");
        let ppid = fields
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok());
        if ppid == Some(parent) {
            pids.push(pid);
        }
    }
    pids
}

/// Run `script`, killing it and every child it forked at [`BOUND`].
fn run_bounded(script: &str) -> (String, String, i32) {
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + BOUND;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => panic!("try_wait failed: {e}"),
        }
        if Instant::now() >= deadline {
            for pid in children_of(child.id()) {
                let _ = kill(Pid::from_raw(pid as i32), SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "`{script}` did not finish in {BOUND:?} — an empty 'do' body is an unbounded loop"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    (
        str::from_utf8(&out.stdout).unwrap().to_string(),
        str::from_utf8(&out.stderr).unwrap().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn run_err(script: &str, msg: &str) {
    let (out, err, code) = run(script);
    assert_eq!((out.as_str(), code), ("", 1), "`{script}` stderr: {err}");
    assert!(err.contains(msg), "`{script}` stderr: {err}");
}

#[test]
fn empty_then_body_is_a_parse_error() {
    // bash 5.3.9: rc 2 for every form (measured). The glued-closer form
    // (`if true; then fi`) is the `;`-before-closer rule's message now
    // (task #301), pinned in `tests/block_closers.rs`.
    run_err("if true; then; fi", "'then' body is empty");
    run_err("if true; then ; fi", "'then' body is empty");
    run_err("if true; then ;; fi", "'then' body is empty");
    run_err("if true\nthen\nfi", "'then' body is empty");
    // A comment is not a command: a comment-only body is empty (bash rc 2).
    run_err("if true; then\n# c\nfi", "'then' body is empty");
    // Task #236: the glued-`fi` form is now the *unclosed block* class — the
    // `fi` behind `b` is an argument and never closes the `if`, so the span
    // swallows the line and the run reports the bare unmatched-keyword parse
    // error (rc 1, bash rc 2 — README "unclosed mid-run construct"). The
    // parse layer's own message for this shape is `missing 'fi'`, pinned in
    // `parse::tests::if_body_before_a_glued_fi_is_a_missing_fi_err`.
    let (out, err, code) = run("if a; then b fi");
    assert_eq!(
        (out.as_str(), code),
        ("", 1),
        "`if a; then b fi` stderr: {err}"
    );
    assert!(err.contains("parse error"), "stderr: {err}");
}

#[test]
fn empty_elif_body_is_a_parse_error() {
    run_err("if a; then b; elif c; then; fi", "'elif' body is empty");
}

#[test]
fn empty_else_body_is_a_parse_error() {
    run_err("if a; then b; else; fi", "'else' body is empty");
    // Used to print `a` with rc 0 (the empty body pinned as None).
    run_err("if true; then echo a; else; fi", "'else' body is empty");
}

#[test]
fn empty_do_body_is_a_parse_error() {
    // The hang forms: bounded child, so a regression panics with the script,
    // never runs the suite into the slow-timeout.
    let (out, err, code) = run_bounded("while true; do; done");
    assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
    assert!(err.contains("'do' body is empty"), "stderr: {err}");
    // Task #301: the glued-`done` form reports the missing `;` first (the
    // `for` message contract, now shared by every block form).
    let (out, err, code) = run_bounded("while true; do done");
    assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
    assert!(err.contains("expected ';' before 'done'"), "stderr: {err}");

    run_err("while true; do\n# c\ndone", "'do' body is empty");
    run_err("until false; do; done", "'do' body is empty");
    run_err("for i in a; do; done", "'do' body is empty");
    // The `;`-before-`done` message fires first (message contract).
    run_err("for i in a; do done", "expected ';' before 'done'");
}

#[test]
fn empty_bodies_recur_in_nested_and_mid_run_blocks() {
    // The inner `if` runs as a cond-list part of the outer condition: the
    // arm text is re-parsed at run time and fails the same way.
    run_err(
        "if true; then if true; then; fi; fi",
        "'then' body is empty",
    );
    // A keyword block written next to a cond operator is a cond part (task #232).
    run_err("true && if true; then; fi", "'then' body is empty");
}

#[test]
fn empty_group_body_stays_its_own_error() {
    // The rule's precedent: `group is empty` (parse/group_block.rs) unchanged.
    run_err("{ ; }", "group is empty");
    run_err("{ }", "group is empty");
}

/// The same rule applied to a function-definition body (task #302, bash
/// `f() { }` rc 2): the body is the group's `compound_list` (POSIX #2.9's
/// `function_def`), so a body with no command is a parse error **at the
/// definition** — the function is never defined, and the rest of the statement
/// list is never reached (`f() { }; f` and `f() { }; echo tail` print nothing).
#[test]
fn empty_function_body_is_a_parse_error() {
    // Every spelling of an empty body, in both definer forms.
    run_err("f() { }", "function definition body is empty");
    run_err("f() {\n}", "function definition body is empty");
    run_err("f() {; }", "function definition body is empty");
    run_err("f() { ;; }", "function definition body is empty");
    // A comment is not a command: a comment-only body is empty (bash rc 2).
    run_err("f() { # c\n}", "function definition body is empty");
    run_err("function f { }", "function definition body is empty");
    run_err("function f() { }", "function definition body is empty");
    // The point of the task: no definition, so the call and the tail of the
    // statement list are never reached (bash stops at the definition, rc 2).
    run_err("f() { }; f", "function definition body is empty");
    run_err("f() { }; echo tail", "function definition body is empty");
    // Every position that carries the definition: the cond part, the group, the
    // arm text (re-parsed at run time), the `case` arm, the pipeline stage, and
    // `time`, whose report must not be printed either.
    run_err("true && f() { }", "function definition body is empty");
    run_err("f() { } && echo x", "function definition body is empty");
    run_err("{ f() { }; }", "function definition body is empty");
    run_err(
        "if true; then f() { }; fi",
        "function definition body is empty",
    );
    run_err(
        "case x in x) f() { } ;; esac",
        "function definition body is empty",
    );
    run_err("echo a | f() { }", "function definition body is empty");
    // The bounded child: an empty body is a parse error, so the loop cannot
    // spin — if this row hangs, the rule regressed.
    let (out, err, code) = run_bounded("while :; do f() { }; break; done");
    assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
    assert!(
        err.contains("function definition body is empty"),
        "stderr: {err}"
    );
    let (out, err, code) = run("time f() { }");
    assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
    assert!(
        err.contains("function definition body is empty") && !err.contains("real"),
        "stderr: {err}"
    );
    // The def-site consequence: the body **slice** is judged at the definition,
    // a nested definition's body is only **text**, judged when the function
    // runs. bash parses the whole script first, so it reports rc 2 for both.
    run_err("f() { f() { }; }; f", "function definition body is empty");
    let (out, err, code) = run("f() { f() { }; }");
    assert_eq!((out.as_str(), code), ("", 0), "stderr: {err}");
}

#[test]
fn heredoc_arm_bodies_are_not_empty() {
    // A body carrying a `<<` operator has tokens: non-empty, and its claimed
    // body region travels with the block (the cond layer's bodies, task #234).
    let (out, err, code) = run("if true; then cat <<T; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr: {err}");
    // The condition-position heredoc pin keeps its `missing 'fi'` message:
    // the closer search runs before any body check (message contract).
    let (_out, err, code) = run("if cat <<X; then\nbody && x\nX\nfi");
    assert_eq!(code, 1);
    assert!(err.contains("missing 'fi'"), "stderr: {err}");
}

#[test]
fn bash_parity_forms_stay_green() {
    let cases: [(&str, &str); 11] = [
        ("if true; then echo a; fi", "a\n"),
        ("if false; then echo a; else echo b; fi", "b\n"),
        (
            "if true; then echo a; elif false; then echo b; else echo c; fi",
            "a\n",
        ),
        ("while false; do echo a; done", ""),
        ("for i in a; do echo a; done", "a\n"),
        // bash rc 0: an empty word list iterates zero times — not a divergence.
        ("for i in; do echo x; done", ""),
        // The function-definition body (task #302): a body with a command — in
        // any spelling — defines and runs exactly as bash does.
        ("f() { :; }; f", ""),
        ("f() { echo a; }; f", "a\n"),
        ("f() {\necho hi\n}; f", "hi\n"),
        // A brace-expanded body is non-empty, and bash's compound-command
        // exception needs no `;` before `}` (the def-site rule is task #424's).
        ("f() { echo {a,b}; }; f", "a b\n"),
        ("f() { if :; then :; fi }", ""),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "`{script}` stderr: {err}");
    }
    // bash accepts empty `case` clause bodies (rc 0): the rule is not applied.
    for script in [
        "case x in a) ;; esac",
        "case x in esac",
        "case x in x) echo m;; esac",
    ] {
        let (out, err, code) = run(script);
        let want = if script.contains("echo m") { "m\n" } else { "" };
        assert_eq!((out.as_str(), code), (want, 0), "`{script}` stderr: {err}");
    }
    // An empty script keeps rc 0 (run_script must stay reachable for "").
    let (out, err, code) = run("");
    assert_eq!((out.as_str(), code), ("", 0), "stderr: {err}");
}

/// A body of **only** the plain `&` is not an empty body: `block_body` accepts
/// it (POSIX's terminator is a word of its own in fdshell's token list), and the
/// body's own scan is what rejects the shape — the invalid split, rc 1 where
/// bash stops with rc 2 (the documented rc-1-vs-2 family). The empty-body
/// messages of this file must not fire on it (task #354).
#[test]
fn ampersand_only_body_is_not_an_empty_body() {
    for script in [
        "if true; then & fi",
        "if true; then ; & fi",
        "while :; do & done",
        "for i in 1; do & done",
        "{ & }",
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 2");
        assert!(
            !err.contains("body is empty"),
            "{script}: the empty-body rule fired: {err}"
        );
    }
    // The untaken arm is parsed, never scanned: rc 0 with the `&` body of an
    // `else` arm that never runs (bash rejects the shape at parse time, rc 2).
    let (out, _err, rc) = run("if true; then :; else & fi");
    assert_eq!((out.as_str(), rc), ("", 0), "bash rc 2");
    // A function-definition body (task #302): the definition is accepted, since
    // the body's own scan — the step that rejects the shape — runs when the
    // function is **called**. bash parses the whole script first, so it stops
    // at the definition with rc 2.
    for script in ["f() { & }", "f() { & ; }", "f() { builtin true & ; }"] {
        let (out, err, rc) = run(script);
        assert_eq!(
            (out.as_str(), rc),
            ("", 0),
            "{script}: bash rc 2 at the def"
        );
        assert!(
            !err.contains("body is empty"),
            "{script}: the empty-body rule fired: {err}"
        );
    }
    // The call is the parse error, and it is not the empty-body message.
    for script in ["f() { & }; f", "f() { & ; }; f"] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 2");
        assert!(
            !err.contains("body is empty"),
            "{script}: the empty-body rule fired: {err}"
        );
    }
}

#![allow(clippy::unwrap_used)]

//! The `;`-before-closer rule for `if`/`while`/`until`/`for` blocks (task
//! #301, POSIX #2.9): a block's closing keyword stands behind a command-list
//! terminator (the `;` byte, a newline, the plain `&`) **or** behind the closer
//! of the compound command that is the body's last statement — bash's grammar
//! makes the `;` after a compound command optional, so
//! `while false; do if :; then :; fi done` and `if true; then { echo a; } fi`
//! run (rc 0, bash 5.3.9, measured 2026-10-23; `master` reported the bare
//! *missing closer* error for every one of them). A block keyword written in
//! argument position is word data and never swallows the closer (the class
//! task #236 pinned); a closer glued straight behind a clause keyword reports
//! `expected ';' before 'fi'`/`'done'`.
//!
//! Every rc below is `bash -c` (5.3.9) against `fdshell -c`. Loop rows use a
//! bounded child ([`run_bounded`], the same helper as `tests/block_bodies.rs`):
//! a block whose closer is mis-paired can run the loop forever, and a
//! regression must panic with the script, not stall the suite.

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

/// 5 s, strictly under nextest's 10 s `slow-timeout` (`.config/nextest.toml`).
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
                "`{script}` did not finish in {BOUND:?} — its block's closer was mis-paired again"
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

/// rc 0 with this stdout.
fn ok(script: &str, want: &str) {
    let (out, err, code) = run(script);
    assert_eq!((out.as_str(), code), (want, 0), "`{script}` stderr: {err}");
}

/// rc 1 carrying this message.
fn err(script: &str, msg: &str) {
    let (out, err, code) = run(script);
    assert_eq!((out.as_str(), code), ("", 1), "`{script}` stderr: {err}");
    assert!(err.contains(msg), "`{script}` stderr: {err}");
}

/// rc 1 carrying this message, under a bounded child.
fn bounded_err(script: &str, msg: &str) {
    let (out, err, code) = run_bounded(script);
    assert_eq!((out.as_str(), code), ("", 1), "`{script}` stderr: {err}");
    assert!(err.contains(msg), "`{script}` stderr: {err}");
}

/// The bash-accepted family: a compound command (an `if`, a group, a function
/// body, a `for`, a `while`, a `case`, a subshell) as the body's last statement
/// closes the block with no `;` in front of the closer. Every row is rc 0 in
/// bash 5.3.9; on `master` every one was rc 1 (`'…' is missing its closing
/// '…'`), because the closer behind a closer was read as an argument.
#[test]
fn a_compound_closer_before_the_closer_is_accepted() {
    for (script, want) in [
        ("while false; do if :; then :; fi done", ""),
        ("until true; do if :; then :; fi done", ""),
        ("while false; do { :; } done", ""),
        ("while false; do f() { :; } done", ""),
        ("while false; do for i in a; do :; done done", ""),
        ("while false; do case x in a) echo c ;; esac done", ""),
        ("while false; do ( : ) done", ""),
        ("while false; do while :; do break; done done", ""),
        ("for i in a; do if :; then echo f; fi done", "f\n"),
        ("for ((i=0;i<1;i++)); do if :; then echo y; fi done", "y\n"),
        ("if true; then if :; then :; fi fi", ""),
        ("if true; then { echo a; } fi", "a\n"),
        ("if true; then case x in a) echo c ;; esac fi", ""),
        ("if true; then for i in a; do :; done fi", ""),
        ("if true; then if :; then echo i; fi fi", "i\n"),
        ("if true; then echo a; else if :; then echo e; fi fi", "a\n"),
        ("until true; do { echo u; } done", ""),
    ] {
        ok(script, want);
    }
    // A `while :` body: bounded, so a mis-paired closer panics with the script.
    let (out, err, code) = run_bounded("if true; then while :; do :; break; done fi");
    assert_eq!((out.as_str(), code), ("", 0), "stderr: {err}");
}

/// The newline spelling of the same shapes (the newline *is* a `;` token, so
/// these were rc 0 before the rule and stay rc 0): the pins that keep the
/// newline path green.
#[test]
fn a_newline_before_the_closer_stays_accepted() {
    ok("while false; do\nif :; then :; fi\ndone", "");
    ok("if true; then\n{ echo a; }\nfi", "a\n");
    ok("if true; then\nif :; then :; fi\nfi", "");
    ok("for i in a; do\nif :; then echo f; fi\ndone", "f\n");
}

/// The same blocks written mid-run (behind a cond operator) and as a pipeline
/// stage: the byte-level scans read the same command-position rule, so their
/// closer is found there too. bash rc 0, `master` rc 1 (`expected 'done'` /
/// `missing 'fi'`).
#[test]
fn mid_run_and_stage_blocks_close_at_their_own_closer() {
    ok("true && while false; do if :; then echo y; fi done", "");
    ok("echo a | while false; do if :; then echo y; fi done", "");
    ok("true && if true; then { echo g; } fi", "g\n");
    ok("true && for i in a; do if :; then echo k; fi done", "k\n");
}

/// The closer glued straight behind a cond operator: bash rc 2, and the rule
/// owns the message (task #358's flipped rows land here — the `&&`/`||` pair is
/// one word, never the plain `&` of the terminator set).
#[test]
fn a_closer_glued_behind_a_cond_operator_is_a_parse_error() {
    err("if true; then echo a && fi", "expected ';' before 'fi'");
    err("if true; then echo a || fi", "expected ';' before 'fi'");
    err(
        "if false; then :; else echo a && fi",
        "expected ';' before 'fi'",
    );
    bounded_err("while false; do : && done", "expected ';' before 'done'");
    bounded_err("while false; do : | done", "expected ';' before 'done'");
}

/// A block keyword in argument position is word data and never swallows the
/// closer (task #236's class, seen from the closer's side): bash rc 2, fdshell
/// rc 1 with the bare unmatched-keyword error — the closer is simply never
/// found, so the block reports it missing.
#[test]
fn a_block_keyword_in_argument_position_never_swallows_the_closer() {
    err(
        "if true; then echo a else fi",
        "'if' is missing its closing 'fi'",
    );
    bounded_err(
        "while false; do echo a else done",
        "'while' is missing its closing 'done'",
    );
    // The argument-position `do` of a `for` list stays the word list's own
    // rule (#273/#306/#310), pinned green: the body's `do` is data.
    ok("for i in a; do echo do; done", "do\n");
}

/// The accepted rc-1-vs-bash-2 family, kept pinned so a future "improvement"
/// cannot move it silently: a closer written behind an argument word, and a
/// closer glued to a redirection (its own word, so no closer predecessor).
/// `master` gives the same rc for every row; the messages are its own.
#[test]
fn the_rc_1_family_stays_a_parse_error() {
    bounded_err(
        "while :; do : done",
        "'while' is missing its closing 'done'",
    );
    err("if true; then b fi", "'if' is missing its closing 'fi'");
    err(
        "for i in a; do echo done",
        "'for' is missing its closing 'done'",
    );
    err(
        "while false; do { :; }>f done",
        "expected ';' before 'done'",
    );
    err(
        "if true; then echo a >f fi",
        "'if' is missing its closing 'fi'",
    );
    err("if true; then echo }; fi", "expected ';' before 'fi'");
    // The POSIX builtin `wait` scores the *zero* keyword delta, so it is not a
    // compound command's end and `done` behind it stays an argument (bash rc 2,
    // `master` rc 1 with this same message; the `Some(0)` row of the closer
    // arm's gate is pinned at `keywords/position/tests.rs`).
    bounded_err(
        "while false; do wait done",
        "'while' is missing its closing 'done'",
    );
}

/// The **accepted divergence** this rule opens: a nested block whose own body
/// is unparseable, written inside a body that never runs. A block body is
/// parsed only when it runs, so the compound-closer rule closes the outer block
/// and the inner statement is never parsed — rc 0 here. bash parses the whole
/// script and stops at rc 2 for every row; `master` rejected every one at rc 1
/// with its *missing-closer* error (`'while' is missing its closing 'done'`,
/// `'if' is missing its closing 'fi'`, and `expected 'done'` for the mid-run
/// and stage twins), i.e. these were the false rejections this task set out to
/// remove, and the acceptance is the rule's own side effect.
///
/// The two halves belong to other tasks, named on each row: the `for … do` word
/// list is #273/#306/#310, the opener/closer pairing (a `while` closed by an
/// `if`) is #309, the glued `||` before a `;;` is the `;;` separator rule of
/// #423, and the *message* of a body that is never parsed is the lazy-body-parse
/// family — parse block bodies eagerly and every row here reports its inner
/// error, matching bash. Pinned so a later task can tell this class from a new
/// break; a dedicated sweep (18 invalid inner statements × 8 block spellings)
/// counts 47 such rows.
#[test]
fn a_never_run_body_is_never_parsed() {
    for script in [
        // The inner `for` list has no `;` before its `do` (standalone: rc 1
        // `expected 'do'`) — the word-list rule of #273/#306/#310.
        "while false; do for i in a do :; done done",
        "if false; then for i in a do :; done fi",
        // The inner `if`/`while` has its closer glued behind a clause keyword
        // (standalone: rc 1 `expected ';' before 'fi'`/`'done'`), and an `if`
        // closing a `while` is #309's type-blind pairing.
        "while false; do if true; then fi done",
        "while false; do while true; do done done",
        // The inner arm body ends in a glued `||` before the `;;`, which the
        // `;;` separator rule of #423 accepts on its own already (standalone:
        // rc 0, bash rc 2).
        "while false; do case x in a) echo a || ;; esac done",
        // The same blocks read by the byte-level scans, mid-run and as a
        // pipeline stage: the scans own the span, the body stays unparsed.
        "true && while false; do if true; then fi done",
        "echo a | while false; do if true; then fi done",
    ] {
        let (out, err, code) = run_bounded(script);
        assert_eq!((out.as_str(), code), ("", 0), "`{script}` stderr: {err}");
    }
    // A body that *runs* does report the inner error, so the acceptance here is
    // lazy body parsing and not the closer rule.
    bounded_err(
        "while :; do if true; then fi done",
        "expected ';' before 'fi'",
    );
}

/// The rule is checked **after** the condition and **before** the bodies, so a
/// real `;`/newline in front of the closer keeps the empty-body message and an
/// empty condition keeps its own error (the message contract of tasks
/// #233/#299, unchanged by #301).
#[test]
fn a_real_terminator_keeps_the_empty_body_and_condition_messages() {
    err("if true; then; fi", "'then' body is empty");
    err("if true; then ; fi", "'then' body is empty");
    err("if true\nthen\nfi", "'then' body is empty");
    err("if true; then\n# c\nfi", "'then' body is empty");
    bounded_err("while true; do; done", "'do' body is empty");
    err("for i in a; do; done", "'do' body is empty");
    err("if ; then; fi", "missing condition");
    bounded_err("while ; do done", "expected condition");
    // The glued-closer rows of the same shapes: the `;` rule speaks first.
    err("if true; then fi", "expected ';' before 'fi'");
    err("if a; then b; else fi", "expected ';' before 'fi'");
    bounded_err("while true; do done", "expected ';' before 'done'");
    err("for i in a; do done", "expected ';' before 'done'");
}

/// The `;;` rows are the separator-side residual task #233 leaves behind: an
/// empty *token range* is what #233 rejects, and a `;;` token is not empty, so
/// these forms stay accepted here (bash rc 2). Task #423 owns that parity;
/// pinned so the split of ownership is visible.
#[test]
fn a_double_semi_body_is_the_separator_rule_not_this_one() {
    ok("if true; then echo a ;; fi", "a\n");
    err("if true; then b ;; fi", "\"b\" not found");
}

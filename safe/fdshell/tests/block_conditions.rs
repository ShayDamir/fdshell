#![allow(clippy::unwrap_used)]

//! Empty block **conditions** (task #299): POSIX #2.9 requires a non-empty
//! `list` as an `if`/`elif`/`while`/`until` condition; bash 5.3.9 rejects every
//! empty form with rc 2 (measured 2026-10-22), fdshell reports its own message
//! with rc 1 (README "Errors"). An empty condition is **true** at run time, so
//! `while ; do echo x; done` used to print forever — every loop pin below runs
//! under a bounded child: a regression must panic, not hang.

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
                "`{script}` did not finish in {BOUND:?} — an empty condition is \
                 true, so the loop is unbounded"
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
fn empty_if_condition_is_a_parse_error() {
    // bash 5.3.9: rc 2 for every form (measured). Before the fix each of these
    // printed its arm with rc 0: an empty condition is true.
    run_err("if ; then echo x; fi", "missing condition");
    run_err("if ;; then echo x; fi", "missing condition");
    // A newline is a `;` token, a comment is dropped: both leave no condition.
    run_err("if\nthen echo x; fi", "missing condition");
    run_err("if\n# c\nthen :; fi", "missing condition");
    // The `&`-terminated spellings (bash rc 2): the `&` is cut from the
    // condition, so the slice trims to nothing.
    run_err("if & then echo x; fi", "missing condition");
    run_err("if ; & then echo x; fi", "missing condition");
    run_err("if ; then echo x; else echo y; fi", "missing condition");
    // Checked before the body: the condition error wins over the empty `then`.
    run_err("if ; then; fi", "missing condition");
}

#[test]
fn empty_elif_condition_is_a_parse_error() {
    // Every arm is parsed, so an empty condition in an arm that never runs is
    // still a parse error (bash rc 2).
    run_err(
        "if true; then echo a; elif ; then b; fi",
        "missing condition",
    );
    run_err(
        "if true; then echo a; elif & then b; fi",
        "missing condition",
    );
    // The `;;` slice keeps the older message (the plain-`;` check runs first).
    run_err("if a; then b; elif ;; then c; fi", "malformed if block");
}

#[test]
fn empty_loop_condition_is_a_parse_error() {
    // The hang forms: bounded child, so a regression panics with the script
    // instead of running the suite into the slow-timeout.
    for script in ["while ; do echo x; done", "while & do :; done"] {
        let (out, err, code) = run_bounded(script);
        assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
        assert!(err.contains("expected condition"), "stderr: {err}");
    }
    run_err("while ;; do :; done", "expected condition");
    run_err("while\n# c\ndo :; done", "expected condition");
    run_err("until ; do echo x; done", "expected condition");
    run_err("until & do :; done", "expected condition");
}

/// An empty condition inside a block runs as part of an outer arm's text: it is
/// re-parsed at run time, reports once, and the script aborts (rc 1, no loop).
#[test]
fn empty_conditions_recur_in_nested_and_mid_run_blocks() {
    run_err("if true; then if ; then :; fi; fi", "missing condition");
    // A keyword block written next to a cond operator is a cond part (task #232).
    run_err("true && if ; then :; fi", "missing condition");
    // The loop form: bounded child, the hang this shape used to be.
    let (out, err, code) = run_bounded("while true; do if ; then :; fi; done");
    assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
    assert!(err.contains("missing condition"), "stderr: {err}");
}

#[test]
fn bash_parity_forms_stay_green() {
    let cases: [(&str, &str); 6] = [
        ("if\ntrue; then echo a; fi", "a\n"),
        ("if true; then echo a; else echo b; fi", "a\n"),
        (
            "if true; then echo a; elif false; then echo b; else echo c; fi",
            "a\n",
        ),
        ("while false; do echo a; done", ""),
        ("until true; do echo a; done", ""),
        ("if true; then if true; then echo a; fi; fi", "a\n"),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "`{script}` stderr: {err}");
    }
}

/// The shapes the rule must **not** reject: a cond-list with an empty **last
/// part** is a non-empty condition — `while : &` stays in fdshell's documented
/// leniency family (bash stops at rc 2 there). A `for` word list and the arith
/// clause are not conditions at all. The `if true && &` row is no longer one of
/// them: its empty last part sits behind an operator, and the cond layer reports
/// that since task **#367** — at fdshell's rc 1, bash's rc 2, so the flag's run is
/// never reached.
#[test]
fn the_neighbouring_leniencies_stay_green() {
    let cases: [(&str, &str); 4] = [
        ("while : & do :; break; done", ""),
        // bash rc 0: an empty word list iterates zero times.
        ("for i in; do echo a; done", ""),
        ("for ((;;)) do :; break; done", ""),
        ("case x in esac", ""),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "`{script}` stderr: {err}");
    }
    let (out, err, code) = run("if true && & then builtin echo a; fi");
    assert_eq!((out.as_str(), code), ("", 1), "bash rc 2: {err}");
    assert!(err.contains("expected command after"), "{err}");
}

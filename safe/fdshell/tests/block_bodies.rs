#![allow(clippy::unwrap_used)]

//! Empty block-arm bodies (task #233): POSIX #2.9 requires a non-empty `list`
//! after `then`, `elif … then`, `else` and `do`; bash 5.3.9 rejects every empty
//! form with rc 2 (measured 2026-10-12), fdshell reports its own message with
//! rc 1 (README "Errors"). The empty-`do` forms used to **run forever**
//! (`while true; do; done` is an unbounded no-op loop), so the two loop pins
//! below run under a bounded child: a regression must panic, not hang.

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
    // bash 5.3.9: rc 2 for every form (measured).
    run_err("if true; then fi", "'then' body is empty");
    run_err("if true; then; fi", "'then' body is empty");
    run_err("if true; then ; fi", "'then' body is empty");
    run_err("if true; then ;; fi", "'then' body is empty");
    run_err("if true\nthen\nfi", "'then' body is empty");
    // A comment is not a command: a comment-only body is empty (bash rc 2).
    run_err("if true; then\n# c\nfi", "'then' body is empty");
    // The glued-`fi` form used to print an accepted empty body (rc 0).
    run_err("if a; then b fi", "'then' body is empty");
}

#[test]
fn empty_elif_body_is_a_parse_error() {
    run_err("if a; then b; elif c; then; fi", "'elif' body is empty");
    // Used to report the misleading `missing 'then'`; the `then` is present.
    run_err("if a; then b; elif c; then fi", "'elif' body is empty");
}

#[test]
fn empty_else_body_is_a_parse_error() {
    run_err("if a; then b; else fi", "'else' body is empty");
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
    let (out, err, code) = run_bounded("while true; do done");
    assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
    assert!(err.contains("'do' body is empty"), "stderr: {err}");

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

#[test]
fn heredoc_arm_bodies_are_not_empty() {
    // A body carrying a `<<` operator has tokens: non-empty, and its claimed
    // body region is appended as before (the single-line block rule).
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
    let cases: [(&str, &str); 6] = [
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

//! bash's errexit gate: `set -e` does not exit for a command "part of any
//! command executed in a `&&` or `||` list except the command following the
//! final `&&` or `||`", and bash applies that exemption to **everything
//! executed inside** a compound sitting in such a position (task #348).
//!
//! Every row here is measured against bash 5.3.9(1) with stdin `/dev/null`,
//! stdout only; the expected values are bash's, and the two owner-pinned rows
//! name their owners (`#490`, `#179`) with bash's value beside them.

#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// Run `script` with cwd `dir`; stdout, stderr and the exit code come back.
fn run_in(dir: &Path, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .current_dir(dir)
        .args(["-c", script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn run(script: &str) -> (String, String, i32) {
    run_in(Path::new("."), script)
}

/// Assert stdout and the exit code of every row, with the script in the
/// failure message (the row is the assertion).
fn assert_rows(rows: &[(&str, &str, i32)]) {
    for (script, stdout, rc) in rows {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            (*stdout, *rc),
            "{script}: stderr={err:?}"
        );
    }
}

/// A per-test scratch dir (pid + counter), removed on drop — the house
/// convention (`LESSONS`: pids are recycled, so the dir must die with its
/// process). The rows that need one open a target under a missing subdirectory.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fdshell-errexit-{}-{}-{}",
            std::process::id(),
            c,
            tag
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// The `for` body runs inside the gate: the body's failure ends the gated part
/// with status 1, the list continues, and the shell does not exit.
#[test]
fn errexit_gate_covers_a_for_body() {
    assert_rows(&[
        // bash: `rc=1` rc 0 (fdshell before the gate: no output, rc 1).
        (
            "set -e; for x in 1; do false; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // The body's own failure is the whole list's status: `||` takes it.
        // bash: `fallback` `rc=0` rc 0.
        (
            "set -e; for x in 1; do false; done || echo fallback; echo rc=$?",
            "fallback\nrc=0\n",
            0,
        ),
        // A `continue` after the failure advances the loop, so the loop ends 0.
        // bash: `after` `rc=0` rc 0.
        (
            "set -e; for x in 1 2; do false; continue; done && echo after; echo rc=$?",
            "after\nrc=0\n",
            0,
        ),
    ]);
}

/// The `while` body is gated too (the `break` leaves the loop with the failing
/// status, the list runs on). bash: `after` `rc=0` rc 0.
#[test]
fn errexit_gate_covers_a_while_body() {
    assert_rows(&[(
        "set -e; while :; do false; break; done && echo after; echo rc=$?",
        "after\nrc=0\n",
        0,
    )]);
}

/// The `until` body is gated. bash: `after` `rc=0` rc 0 (fdshell agreed today
/// through the `until` condition's exemption, pinned so the gate cannot
/// regress it into the loop-body path).
#[test]
fn errexit_gate_covers_an_until_body() {
    assert_rows(&[(
        "set -e; until :; do false; break; done && echo after; echo rc=$?",
        "after\nrc=0\n",
        0,
    )]);
}

/// The arithmetic `for` propagates the gate through its body's own cond list.
/// bash: `rc=1` rc 0.
#[test]
fn errexit_gate_covers_an_arithmetic_for_body() {
    assert_rows(&[(
        "set -e; for ((i=0;i<1;i++)); do false; done && echo after; echo rc=$?",
        "rc=1\n",
        0,
    )]);
}

/// A `{ …; }` group in a gated position is exempt, including a group whose body
/// keeps running after its own failure (`inner` prints, then the list runs on).
#[test]
fn errexit_gate_covers_a_group_body() {
    assert_rows(&[
        // bash: `rc=1` rc 0.
        ("set -e; { false; } && echo after; echo rc=$?", "rc=1\n", 0),
        // bash: `inner` `tail` `rc=0` rc 0 — the body's own statements run on.
        (
            "set -e; { for x in 1; do false; done; echo inner; } && echo tail; echo rc=$?",
            "inner\ntail\nrc=0\n",
            0,
        ),
        // The non-final part is `{ false; }`; the final part never runs.
        // bash: `rc=1` rc 0.
        (
            "set -e; { false; } && { echo a; false; }; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// An `if` then-body is gated; the *condition* keeps its own, older exemption.
#[test]
fn errexit_gate_covers_an_if_then_body() {
    assert_rows(&[
        // bash: `rc=1` rc 0.
        (
            "set -e; if true; then false; fi && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // A gated loop body that runs an `if` whose then-body fails.
        // bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do if true; then false; fi; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// The `else` body is gated as well. bash: `rc=1` rc 0.
#[test]
fn errexit_gate_covers_an_if_else_body() {
    assert_rows(&[(
        "set -e; if false; then :; else false; fi && echo after; echo rc=$?",
        "rc=1\n",
        0,
    )]);
}

/// A `case` arm body is gated, in the matching arm and in the arm the word
/// selects after a non-matching one.
#[test]
fn errexit_gate_covers_a_case_arm() {
    assert_rows(&[
        // bash: `rc=1` rc 0.
        (
            "set -e; case 1 in 1) false;; esac && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // bash: `rc=1` rc 0.
        (
            "set -e; case 1 in 2) true;; 1) false;; esac && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // A gated loop body running a failing `case` arm. bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do case 1 in 1) false;; esac; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// A function-call frame in a gated position is exempt, including a function
/// defined inside a gated loop body.
#[test]
fn errexit_gate_covers_a_function_frame() {
    assert_rows(&[
        // bash: `rc=1` rc 0.
        (
            "set -e; f() { false; }; f && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // The definition runs inside the gated body; its call is the body's
        // failing command. bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do f() { false; }; f; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// The gate reaches nested compounds: a group in a loop body, a loop in a loop
/// body — the innermost failure is exempt all the same.
#[test]
fn errexit_gate_covers_a_nested_compound() {
    assert_rows(&[
        // bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do { false; }; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do for y in 1; do false; done; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// The gate reaches an **inner** AND/OR list: inside a gated body the inner
/// list's final command is exempt as well, because bash's exemption is a
/// property of the execution context, not of one list.
#[test]
fn errexit_gate_covers_a_nested_and_or_list() {
    assert_rows(&[
        // The inner list's failure is data for the body; `inner=1` prints.
        // bash: `inner=1` `after` `rc=0` rc 0.
        (
            "set -e; for x in 1; do false && echo a; echo inner=$?; done && echo after; echo rc=$?",
            "inner=1\nafter\nrc=0\n",
            0,
        ),
        // bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do true && false; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// The gate is symmetric: it covers the `||` side as well, including a gated
/// function frame that hands its failure to a `||` fallback.
#[test]
fn errexit_gate_on_the_or_side() {
    assert_rows(&[
        // bash: `after` `rc=0` rc 0.
        (
            "set -e; { false; } || echo after; echo rc=$?",
            "after\nrc=0\n",
            0,
        ),
        // bash: `fallback` `rc=0` rc 0.
        (
            "set -e; f() { false; }; f || echo fallback; echo rc=$?",
            "fallback\nrc=0\n",
            0,
        ),
        // The final `false` follows the final `||`, so the shell exits there:
        // `a` prints, rc 1, `echo rc=$?` never runs. bash: `a` rc 1.
        (
            "set -e; { false; } || { echo a; false; }; echo rc=$?",
            "a\n",
            1,
        ),
    ]);
}

/// A three-part list gates every part but the last: the failing gated part
/// short-circuits the rest, and the list ends with the failing status.
#[test]
fn errexit_gate_in_a_three_part_list() {
    assert_rows(&[
        // bash: `rc=1` rc 0.
        (
            "set -e; { false; } && echo a && echo b; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // `&& echo a` is skipped, `|| echo b` runs. bash: `b` `rc=0` rc 0.
        (
            "set -e; for x in 1; do false; done && echo a || echo b; echo rc=$?",
            "b\nrc=0\n",
            0,
        ),
        // bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do false; done && echo a && echo b; echo rc=$?",
            "rc=1\n",
            0,
        ),
        // The gated compound fails, so the whole `&&` chain short-circuits.
        // bash: `rc=1` rc 0.
        (
            "set -e; for x in 1; do false; done && for y in 1; do false; done; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// The exemption suppresses the exit, never the status: every gated row above
/// ends the list with `1`, so `rc=$?` prints 1.
#[test]
fn errexit_gate_keeps_the_failing_status() {
    assert_rows(&[
        (
            "set -e; for x in 1; do false; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        ("set -e; { false; } && echo after; echo rc=$?", "rc=1\n", 0),
        (
            "set -e; case 1 in 1) false;; esac && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        (
            "set -e; if true; then false; fi && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        (
            "set -e; f() { false; }; f && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
        (
            "set -e; for ((i=0;i<1;i++)); do false; done && echo after; echo rc=$?",
            "rc=1\n",
            0,
        ),
    ]);
}

/// The gate ends with the list: the restore is what makes the **next**
/// statement's failure fatal again (the shape of the unit pin
/// `errexit_gate_is_restored_after_the_list`).
#[test]
fn gate_restored_at_the_list_end_exits_on_the_next_statement() {
    assert_rows(&[(
        "set -e; for x in 1; do false; done && echo a; false; echo tail",
        "",
        1,
    )]);
}

/// A compound that is the **final** element of the list, or a compound that is
/// not in a list at all, keeps exiting on its body's failure.
#[test]
fn ungated_compound_still_exits() {
    assert_rows(&[
        // The compound follows the final `&&`: fatal. bash: rc 1, no output.
        (
            "set -e; true && for x in 1; do false; done; echo rc=$?",
            "",
            1,
        ),
        ("set -e; true && { false; }; echo rc=$?", "", 1),
        (
            "set -e; for x in 1; do true && false; done; echo rc=$?",
            "",
            1,
        ),
        // No list at all: the body's failure is the statement's own.
        ("set -e; { false; }; echo tail", "", 1),
        ("set -e; for x in 1; do false; done; echo tail", "", 1),
        ("set -e; case 1 in 1) false;; esac; echo tail", "", 1),
        ("set -e; if true; then false; fi; echo tail", "", 1),
        ("set -e; f() { false; }; f; echo tail", "", 1),
        ("set -e; while :; do false; break; done; echo tail", "", 1),
    ]);
}

/// The shapes that already agreed before the gate stays green, including task
/// #164's `while true; do false && echo a; break; done` and the exempt
/// conditions of `if`/`elif`/`until`.
#[test]
fn existing_errexit_pins_stay_green() {
    assert_rows(&[
        // An exempt condition is not the gate: `if false` leaves status 0.
        (
            "set -e; if false; then :; fi && echo after; echo rc=$?",
            "after\nrc=0\n",
            0,
        ),
        (
            "set -e; if false; then :; elif false; then :; fi && echo after; echo rc=$?",
            "after\nrc=0\n",
            0,
        ),
        // The plain list rules (LESSONS: a skipped `&&` tail needs the `ran` guard).
        ("set -e; false && echo x; echo rc=$?", "rc=1\n", 0),
        // `source` of an empty file succeeds, so the `&&` tail runs.
        (
            "set -e; source /dev/null && echo after; echo rc=$?",
            "after\nrc=0\n",
            0,
        ),
        // The pipeline's **last** stage decides; `false | true` is rc 0.
        ("set -e; false | true; echo after", "after\n", 0),
        // Task #164's shape: the gated inner list's failure is data for the loop.
        (
            "set -e; while true; do false && echo a; break; done; echo after",
            "after\n",
            0,
        ),
    ]);
}

/// A gated compound whose **redirection** cannot open its target: bash prints
/// `rc=1` at rc 0 (the failing open is the compound's status, the gate exempts
/// it, the list runs on). fdshell aborts the whole script at rc 1, with its own
/// `redirection failed` report — the abort is the
/// `#179` divergence ("a redirect-open failure aborts the remaining commands;
/// POSIX fails only the command"), not an errexit gate gap: the same shape
/// **without** `set -e` also aborts here (`tail` printed by bash, nothing by
/// fdshell), and the ungated row agrees only by coincidence.
#[test]
fn a_gated_compound_with_an_unopenable_redirect_keeps_its_owner_179() {
    let dir = Scratch::new("redirect");
    for (script, out, rc) in [
        // bash: `rc=1` rc 0. fdshell (pinned, owner #179): no output, rc 1.
        (
            "set -e; for x in 1; do true; done > nodir/x && echo after; echo rc=$?",
            "",
            1,
        ),
        // bash: `tail` rc 0 — the errexit-free control row of the same abort.
        ("for x in 1; do true; done > nodir/x; echo tail", "", 1),
        // The ungated twin agrees with bash's rc (both exit 1): the abort, not
        // the gate, decides it. bash: rc 1, no output.
        (
            "set -e; for x in 1; do true; done > nodir/x; echo rc=$?",
            "",
            1,
        ),
    ] {
        let (o, err, code) = run_in(&dir, script);
        assert_eq!((o.as_str(), code), (out, rc), "{script}: stderr={err:?}");
    }
}

/// A failing command in a **non-final pipeline stage** is exempt in bash ("any
/// command in a pipeline but the last"). Owner **#490**: fdshell exits. Pinned
/// as measured so the row cannot drift silently; bash's value is in the comment.
#[test]
fn a_nonfinal_pipeline_stage_keeps_its_owner_490() {
    assert_rows(&[
        // bash: `rc=0` rc 0. fdshell (pinned, owner #490): no output, rc 1.
        (
            "set -e; for x in 1; do false; done | cat; echo rc=$?",
            "",
            1,
        ),
        // The stage's own last-command status is exempt in bash too:
        // bash `rc=0` rc 0, fdshell (owner #490) no output rc 1.
        (
            "set -e; for x in 1; do false; done | cat >/dev/null; echo rc=$?",
            "",
            1,
        ),
    ]);
}

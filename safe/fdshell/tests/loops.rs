#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
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

/// `break 2` exits both nested loops: the first inner iteration runs, then both
/// loops exit. Measured against `bash -c` (5.3.9): `iterations:1`.
#[test]
fn break_2_exits_both_nested_loops() {
    let (out, err, code) = run(
        r#"i=0; for a in 1 2 3; do for b in 1 2 3; do i=$((i+1)); break 2; done; done; echo "iterations:$i""#,
    );
    assert_eq!(code, 0);
    assert_eq!(out, "iterations:1\n");
    assert_eq!(err, "");
}

/// `continue 2` advances the outer `while` (the `for` is the inner level). The
/// guarded `while` terminates. Measured: `j=3`, `inner` never printed.
#[test]
fn continue_2_advances_the_outer_loop() {
    let (out, code) = {
        let (out, _err, code) = run(
            r#"j=0; while [ $j -lt 3 ]; do for c in 1 2; do j=$((j+1)); continue 2; done; echo inner; done; echo j=$j"#,
        );
        (out, code)
    };
    assert_eq!(code, 0);
    assert_eq!(out, "j=3\n");
}

/// `break 5` in depth 1 is clamped to 1, silently (no warning). Measured: `j=1`.
#[test]
fn break_count_clamped_to_depth_silently() {
    let (out, err, code) = run("j=0; for a in 1 2 3; do j=$((j+1)); break 5; done; echo j=$j");
    assert_eq!(code, 0);
    assert_eq!(out, "j=1\n");
    assert_eq!(err, "");
}

/// `continue 5` in depth 1 is clamped to 1, so every iteration runs. Measured: `i=4`.
#[test]
fn continue_count_clamped_runs_every_iteration() {
    let (out, code) = {
        let (out, _err, code) =
            run("i=0; for a in 1 2 3 4; do i=$((i+1)); continue 5; done; echo i=$i");
        (out, code)
    };
    assert_eq!(code, 0);
    assert_eq!(out, "i=4\n");
}

/// `case` is not a loop level: `break 2` through a `case` exits the two loops.
/// Measured: `m=1`, `after` never printed.
#[test]
fn break_2_through_case_exits_both_loops() {
    let (out, code) = {
        let (out, _err, code) = run(
            r#"m=0; for a in 1 2 3; do case $a in 1) for b in 1 2 3; do m=$((m+1)); break 2; done;; esac; echo after; done; echo m=$m"#,
        );
        (out, code)
    };
    assert_eq!(code, 0);
    assert_eq!(out, "m=1\n");
}

/// `until` is a loop level: `break 2` exits the `until` and the outer `for`.
/// Measured: `i=1`.
#[test]
fn break_2_exits_until_and_outer_for() {
    let (out, code) = {
        let (out, _err, code) = run(
            "i=0; for a in 1 2 3; do until false; do i=$((i+1)); break 2; done; done; echo i=$i",
        );
        (out, code)
    };
    assert_eq!(code, 0);
    assert_eq!(out, "i=1\n");
}

/// `while` is a loop level: `continue 2` advances the outer `for`. Measured: `i=3`.
#[test]
fn continue_2_in_while_advances_outer_for() {
    let (out, code) = {
        let (out, _err, code) = run(
            "i=0; for a in 1 2 3; do while :; do i=$((i+1)); continue 2; done; done; echo i=$i",
        );
        (out, code)
    };
    assert_eq!(code, 0);
    assert_eq!(out, "i=3\n");
}

/// `source` shares the caller's loop depth, so `break 2` in a sourced file exits
/// both loops. Measured: only `end` printed (bash agrees).
#[test]
fn source_break_2_exits_both_loops() {
    let path = std::env::temp_dir().join(format!("fdshell_loops_source_{}", std::process::id()));
    std::fs::write(&path, "break 2\n").unwrap();
    let script = format!(
        "for a in 1 2; do for b in 1 2; do source {}; echo inner; done; echo outer; done; echo end",
        path.to_str().unwrap()
    );
    let (out, code) = {
        let (out, _err, code) = run(&script);
        (out, code)
    };
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0);
    assert_eq!(out, "end\n");
}

/// Accepted divergence: the function body is NOT a loop boundary here — `break 2`
/// in a function crosses it and exits the caller's loops. bash warns "only
/// meaningful in a … loop" and keeps running. Task #203 owns this change; this
/// test is its drift alarm (pinning current fdshell behavior).
#[test]
fn break_2_in_function_body_crosses_the_boundary_current_fdshell_behavior() {
    let (out, code) = {
        let (out, _err, code) = run(
            r#"f() { break 2; }; for a in 1 2; do for b in 1 2; do f; echo inner; done; echo outer; done; echo end"#,
        );
        (out, code)
    };
    assert_eq!(code, 0);
    assert_eq!(out, "end\n");
}

/// Accepted divergence: count 0 is rejected (rc 1, parse abort) where bash warns
/// "loop count out of range" and breaks one level. README "Accepted divergences".
#[test]
fn break_zero_is_rejected() {
    let (_out, err, code) = run("for a in 1 2 3; do break 0; echo x; done; echo done");
    assert_eq!(code, 1);
    assert!(err.contains("break/continue count must be a positive integer"));
}

/// Accepted divergence: negative count is rejected (rc 1) where bash warns and
/// breaks one level.
#[test]
fn break_negative_is_rejected() {
    let (_out, err, code) = run("for a in 1 2 3; do break -1; echo x; done; echo done");
    assert_eq!(code, 1);
    assert!(err.contains("break/continue count must be a positive integer"));
}

/// Accepted divergence: a non-numeric count is rejected here with rc 1, where
/// bash aborts with rc 2 ("numeric argument required").
#[test]
fn break_non_numeric_is_rejected() {
    let (_out, err, code) = run("for a in 1 2 3; do break abc; echo x; done; echo done");
    assert_eq!(code, 1);
    assert!(err.contains("break/continue count must be a positive integer"));
}

/// `break 2 3` (more than one argument) aborts with rc 1, matching bash's rc 1
/// (the message differs: bash says "too many arguments").
#[test]
fn break_extra_arguments_aborts_rc1() {
    let (_out, err, code) = run("for a in 1 2 3; do break 2 3; done; echo end");
    assert_eq!(code, 1);
    assert!(err.contains("break takes at most one argument"));
}

/// Accepted divergence: the count is not expanded, so `break $n` is rejected
/// (rc 1), where bash expands `$n` to 2 and breaks two levels.
#[test]
fn break_count_is_not_expanded() {
    let (_out, err, code) = run("n=2; i=0; for a in 1 2 3; do i=$((i+1)); break $n; done; echo $i");
    assert_eq!(code, 1);
    assert!(err.contains("break/continue count must be a positive integer"));
}

/// `break 2` outside any loop aborts with rc 1 here, where bash warns "only
/// meaningful in a … loop" and continues with rc 0. This is fdshell's existing
/// accepted outside-loop behavior (README, task #99 rule 7).
#[test]
fn break_2_outside_loop_aborts_rc1() {
    let (_out, err, code) = run("break 2; echo end");
    assert_eq!(code, 1);
    assert!(err.contains("break"));
}

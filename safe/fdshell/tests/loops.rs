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
    assert!(err.contains("'break' is not inside a loop"));
}

/// Accepted divergence + rc change vs master: the depth-0 bail also fires for a
/// `break` nested in an `if` **condition**. master printed `y` `after` rc 0 (the
/// condition's control was dropped), bash warns "only meaningful in a … loop"
/// with rc 0; here rc 1 with the outside-loop message. README "Accepted
/// divergences"; the condition-control fix is task #205.
#[test]
fn break_in_if_condition_at_depth_zero_aborts_rc1() {
    let (_out, err, code) = run("if break; then echo y; fi; echo after");
    assert_eq!(code, 1);
    assert!(err.contains("'break' is not inside a loop"));
}

/// Same shape with `continue`: master printed `after` rc 0, bash warns rc 0,
/// here rc 1 with the outside-loop message.
#[test]
fn continue_in_if_condition_at_depth_zero_aborts_rc1() {
    let (_out, err, code) = run("if continue; then :; fi; echo after");
    assert_eq!(code, 1);
    assert!(err.contains("'continue' is not inside a loop"));
}

// Task #130: a quoted region in a loop condition is one word — its `;` does not
// end the statement, so the condition runs once. (`&&` in a `while` condition is
// task #137 and stays out of these shapes.)
#[test]
fn quoted_region_in_a_loop_condition_runs_once() {
    let (out, err, code) = run(r#"while echo "$(echo "x; y")"; do break; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "x; y\n");
}

/// A quoted region in a for list is one word (task #130) and is *substituted*
/// (task #131): `"a$(printf " b")c"` expands to `a bc` and iterates once.
/// Measured against `bash -c` (5.3.9): `[a bc]`.
#[test]
fn quoted_region_in_a_for_list_is_one_word() {
    let (out, err, code) = run(r#"for w in "a$(printf " b")c"; do echo "[$w]"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(err, "");
    assert_eq!(out, "[a bc]\n");
}

/// A for-list word goes through the argument substitution pipeline (task
/// #131): `$x`, `"$x"` and `${x}b` all expand. Measured: `[a]`, `[a]`, `[ab]`.
#[test]
fn for_list_word_expands_variable() {
    let (out, err, code) = run(r#"x=a; for w in $x; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a]\n");
    let (out, _err, code) = run(r#"x=a; for w in "$x"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n");
    let (out, _err, code) = run(r#"x=a; for w in ${x}b; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[ab]\n");
}

/// An unquoted for-list expansion is split on `IFS` and each field is one
/// iteration; `IFS=` disables the split. Measured: `[a] [b]`, `[a] [b]`,
/// `[a b]`.
#[test]
fn for_list_unquoted_expansion_splits_on_ifs() {
    let (out, err, code) = run(r#"x="a b"; for w in $x; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a]\n[b]\n");
    let (out, _err, code) = run(r#"IFS=:; x=a:b; for w in $x; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n[b]\n");
    let (out, _err, code) = run(r#"IFS=; x="a b"; for w in $x; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]\n");
}

/// A leading `~` in a for-list word expands to `$HOME`; `~` anywhere else in
/// the word stays literal. Compared against `printf "%s" "$HOME"`, never a
/// hardcoded home path.
#[test]
fn for_list_word_expands_leading_tilde() {
    let (home, _err, code) = run(r#"printf "[%s]" "$HOME""#);
    assert_eq!(code, 0);
    let (out, err, code) = run(r#"for w in ~; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, format!("{home}\n"));
    let (out, _err, code) = run(r#"for w in a~b; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a~b]\n");
}

/// A quoted command substitution is exactly one field (closes #132): the
/// output's whitespace is not a split point. Unquoted, the same output splits
/// on `IFS` into one iteration per field. Measured: `[x y]` (one iteration);
/// `[a] [b] [c]`.
#[test]
fn for_list_quoted_cmd_subst_is_one_word() {
    let (out, err, code) = run(r#"for w in "$(echo a b)"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a b]\n");
    let (out, _err, code) = run(r#"for w in "$(echo "x" "y")"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[x y]\n");
    let (out, _err, code) = run(r#"for w in $(echo "a b" c); do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n[b]\n[c]\n");
}

/// `$@` in a for list iterates over the positionals; quoted `"$@"` keeps a
/// positional containing a space whole. `set --` runs at top level — inside a
/// function the name leaks into `$@` (task #134). Measured: `[a] [b]` and
/// `[a b] [c]`.
#[test]
fn for_list_dollar_at_iterates_positionals() {
    let (out, err, code) = run(r#"set -- a b; for w in $@; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a]\n[b]\n");
    let (out, _err, code) = run(r#"set -- "a b" c; for w in "$@"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]\n[c]\n");
}

/// Accepted divergence (task #219): `"$@"` with no positionals is one empty
/// iteration here, where bash gives none. The for list inherits the rule from
/// `substitute_args`' quoted-empty rule; pinned as a drift alarm.
#[test]
fn for_list_empty_dollar_at_is_one_iteration() {
    let (out, _err, code) = run(r#"set --; for w in "$@"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n");
}

/// The whole-word backtick arm keeps working (task #216 owns moving backticks
/// into the argument pipeline; this is its drift alarm), including the
/// quoted-empty rule. Measured: `[hi]`, `[]` (one iteration), no iteration.
#[test]
fn for_list_whole_word_backtick_still_expands() {
    let (out, err, code) = run(r#"for w in `echo hi`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[hi]\n");
    let (out, _err, code) = run(r#"for w in "`true`"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n");
    let (out, _err, code) = run(r#"for w in "`echo a`"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n");
    let (out, _err, code) = run(r#"for w in `true`; do echo hit; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "");
}

/// A fully quoted command substitution in a for list is ONE word: its spaces
/// and newlines are data, never split points (task #132, measured against
/// `bash -c` 5.3.9 with `od -c`). `IFS` is irrelevant there — the third case
/// sets it to `,` and the word still stays whole.
/// Measured: `[a b]`, `[x LF y]` (one iteration), `[a b]`.
#[test]
fn for_list_quoted_backtick_is_never_split() {
    let (out, err, code) = run(r#"for w in "`echo a b`"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a b]\n");
    // The newline comes from a third command in the body: no escape is needed
    // (a `\n` inside a quoted backtick body loses its backslash here — #216).
    let (out, _err, code) =
        run(r#"for w in "`printf x; echo; printf y`"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[x\ny]\n");
    let (out, _err, code) = run(r#"IFS=,; for w in "`echo a b`"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]\n");
}

/// An unquoted whole-word backtick's output is split on `IFS` exactly like any
/// other expansion (task #132): every IFS byte delimits (space, TAB, LF, and
/// non-whitespace bytes such as `:`), `IFS=` disables the split, CR is not an
/// `IFS` whitespace byte, and runs of `IFS` whitespace collapse.
/// Measured: `[a b]`, `[a] [b]`, `[a⇥b]`, `[a⇥b]`, `[a\rb]`, `[x] [y]`, `[a] [b]`.
#[test]
fn for_list_backtick_output_splits_on_ifs() {
    let (out, err, code) = run(r#"IFS=; for w in `echo a b`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a b]\n");
    let (out, _err, code) =
        run(r#"IFS=:; for w in `printf %s "a:b"`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n[b]\n");
    // `%b` turns the escape into a real TAB in both shells; `%s` keeps it text.
    let (out, _err, code) =
        run(r#"IFS=" "; for w in `printf %b "a\tb"`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a\tb]\n");
    let (out, _err, code) =
        run(r#"IFS=:; for w in `printf %b "a\tb"`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a\tb]\n");
    let (out, _err, code) = run(r#"for w in `printf "a\rb"`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a\rb]\n");
    let (out, _err, code) = run(r#"for w in `printf "x\ny"`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[x]\n[y]\n");
    let (out, _err, code) = run(r#"for w in `echo a  b`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a]\n[b]\n");
}

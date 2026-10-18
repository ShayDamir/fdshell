#![allow(clippy::unwrap_used)]

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

/// A loop-control regression runs forever, so the child is bounded: 5 s is
/// strictly under nextest's 10 s `slow-timeout` (`.config/nextest.toml`), which
/// makes the diagnostic panic below — not a bare timeout — report the regression.
const BOUND: Duration = Duration::from_secs(5);

/// All pids whose parent is `parent` (the same helper as `tests/pipeline.rs`:
/// every integration-test file is its own binary, so the helper is local).
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
        // "pid (comm) state ppid …" — comm may contain spaces and parens.
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

/// Run `script`, killing it and every child it forked at [`BOUND`]. Every
/// condition-control shape below hangs when the fix regresses (a dropped signal
/// makes the loop re-test its condition forever), so none of them may use the
/// unbounded [`run`].
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
                "`{script}` did not finish in {BOUND:?} — a loop-control signal raised in a condition was dropped, so the loop re-tests its condition forever"
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
// task #137, now accepted — see `while_and_in_condition_runs_the_cond_list`.)
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

/// Task #282: the for-list tilde follows the `HOME` **shell variable** — a
/// `HOME=…` assignment moves `~` exactly as it moves `$HOME` (POSIX #2.6.1,
/// bash 5.3.9: `HOME=/tmp/H; for w in ~/x; do …` iterates over `/tmp/H/x`).
#[test]
fn for_list_tilde_follows_the_shell_variable_home() {
    for (script, want) in [
        (
            r#"HOME=/tmp/H; for w in ~/x; do printf "[%s]\n" "$w"; done"#,
            "[/tmp/H/x]\n",
        ),
        (
            r#"HOME=/tmp/H; for w in ~ "$HOME"; do printf "[%s]\n" "$w"; done"#,
            "[/tmp/H]\n[/tmp/H]\n",
        ),
        // A `HOME` assigned in a prefix position of the `for` word is not a
        // shell assignment; the for-list word is substituted in the shell.
        (
            r#"HOME=/tmp/H; for w in ~x; do printf "[%s]\n" "$w"; done"#,
            "[~x]\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}");
    }
}

/// A **quoted** leading `~` is a literal byte (POSIX expands tildes before
/// quote removal, and bash's `for w in "~"` iterates over `~`) — task #269's
/// mask gate in `substitute/tilde.rs`. With the empty mask fdshell expanded
/// `$HOME`.
///
/// Task #281 widens the gate: a quoted character anywhere in the tilde-prefix
/// (the bytes from `~` through the first `/`, the `/` included) suppresses the
/// expansion, so bash's `for w in ~"/*"` iterates over `~/*` — not `$HOME/*`.
#[test]
fn for_list_quoted_leading_tilde_is_literal() {
    let (out, err, code) = run(r#"for w in "~"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[~]\n", "bash iterates over the literal ~");
    let (out, _err, code) = run(r#"for w in "~" x; do printf "[%s]" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[~][x]");
    let (out, _err, code) = run(r#"for w in ~"/*"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[~/*]\n", "bash iterates over [~/*]");
    let (out, _err, code) = run(r#"for w in ~"/x" y; do printf "[%s]" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[~/x][y]", "bash iterates over [~/x] then [y]");
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
/// positional containing a space whole. Measured: `[a] [b]` and `[a b] [c]`.
#[test]
fn for_list_dollar_at_iterates_positionals() {
    let (out, err, code) = run(r#"set -- a b; for w in $@; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a]\n[b]\n");
    let (out, _err, code) = run(r#"set -- "a b" c; for w in "$@"; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[a b]\n[c]\n");
}

/// Bash parity (task #219): `"$@"` with no positionals is **no iteration** —
/// the fully quoted whole-word `$@` *is* the positional word list, so an
/// empty deque produces no word at all. The for list inherits the rule from
/// `substitute_args`; the pair with `"$*"` shows the join word still iterates
/// once. Measured: `end`, `hitend`.
#[test]
fn for_list_empty_dollar_at_is_no_iteration() {
    let (out, err, code) = run(r#"set --; for w in "$@"; do printf hit; done; echo end"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    let (out, err, code) = run(r#"set --; for w in "$@" "$*"; do printf hit; done; echo end"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "hitend\n");
}

/// Whole-word backticks in a for list work through the argument pipeline
/// (task #216 deleted the whole-word arm that used to own this), including the
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
    // The newline comes from a third command in the body: a `\n` escape cannot
    // survive it — the child re-parses the body with its own tokenizer, whose
    // unquoted `\n` pair folds to a literal `n` (both shells print `[xny]`).
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

/// A for-list word that performed no expansion is one iteration (task #133,
/// POSIX 2.6.5): its own `IFS` bytes are not split points.
/// Measured against `bash -c` 5.3.9: `[a,b]` (one iteration).
#[test]
fn for_list_literal_word_with_ifs_byte_is_one_iteration() {
    let (out, err, code) = run(r#"IFS=,; for w in a,b; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a,b]\n");
}

/// A trailing `IFS` delimiter at the end of an expansion produces no trailing
/// empty field (task #133): the whole-word backtick's output goes through the
/// same splitter. Measured: `[a]` (one iteration), `[]` `[]` (two).
#[test]
fn for_list_backtick_output_elides_trailing_empty_field() {
    let (out, err, code) = run(r#"IFS=,; for w in `echo "a,"`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "[a]\n");
    let (out, _err, code) = run(r#"IFS=,; for w in `echo ",,"`; do printf "[%s]\n" "$w"; done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "[]\n[]\n");
}

/// Task #220 (fixed free by task #216): a whole-word backtick's fields are
/// pathname-expanded, because the word now runs through the argument pipeline,
/// whose `push_word` globs every field. The `$( … )` form is the reference
/// shape — both give the same fields. Measured against `bash -c` 5.3.9 in a
/// directory with `ab ac zz`: `[ab]`, `[ac]`, `[zz]`.
#[test]
fn for_list_backtick_fields_are_globbed() {
    let dir = std::env::temp_dir().join(format!("fdshell-loops-glob-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["ab", "ac", "zz"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    let dir = dir.to_str().unwrap().to_string();
    for script in [
        r#"for w in `echo "*"`; do printf "[%s]\n" "$w"; done"#,
        r#"for w in $(echo "*"); do printf "[%s]\n" "$w"; done"#,
    ] {
        let output = Command::new(BIN)
            .current_dir(&dir)
            .args(["-c", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        let out = str::from_utf8(&output.stdout).unwrap().to_string();
        let err = str::from_utf8(&output.stderr).unwrap().to_string();
        assert_eq!(output.status.code().unwrap_or(-1), 0, "{script:?}: {err}");
        assert_eq!(out, "[ab]\n[ac]\n[zz]\n", "{script:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Task #220: the backtick fields reach the same `glob::expand` as the
/// `$( … )` fields, so the for-list surface honours the shell's glob options on
/// both forms. A non-matching pattern is dropped by `nullglob`, and `set -f`
/// (noglob) passes the pattern through verbatim without touching the
/// filesystem. Measured against `bash -c` 5.3.9 in a directory with `ab ac zz`:
/// both forms print nothing with `nullglob`, and `[*]` with `set -f`.
#[test]
fn for_list_backtick_fields_respect_glob_options() {
    let dir = std::env::temp_dir().join(format!("fdshell-loops-globopt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["ab", "ac", "zz"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    let dir = dir.to_str().unwrap().to_string();
    for script in [
        r#"shopt -s nullglob; for w in `echo "nomatch*"`; do printf "[%s]\n" "$w"; done"#,
        r#"shopt -s nullglob; for w in $(echo "nomatch*"); do printf "[%s]\n" "$w"; done"#,
        r#"set -f; for w in `echo "*"`; do printf "[%s]\n" "$w"; done"#,
        r#"set -f; for w in $(echo "*"); do printf "[%s]\n" "$w"; done"#,
    ] {
        let output = Command::new(BIN)
            .current_dir(&dir)
            .args(["-c", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        let out = str::from_utf8(&output.stdout).unwrap().to_string();
        let err = str::from_utf8(&output.stderr).unwrap().to_string();
        let want = if script.contains("nullglob") {
            ""
        } else {
            "[*]\n"
        };
        assert_eq!(output.status.code().unwrap_or(-1), 0, "{script:?}: {err}");
        assert_eq!(out, want, "{script:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Task #137: `&&` / `||` in a `while`/`until` condition is the loop's own cond
/// list, exactly as bash runs it — the block text is never split at it (master:
/// `expected 'do'` / `expected 'done'` rc 1). Measured against `bash -c` 5.3.9:
/// `hi` `rc=0`, `rc=0`, and the short-circuit `i=0`.
#[test]
fn while_and_in_condition_runs_the_cond_list() {
    let (out, err, code) = run("while [ 1 ] && [ 1 ]; do echo hi; break; done; echo rc=$?");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "hi\nrc=0\n");
    let (out, err, code) =
        run("i=0; while [ $i -lt 5 ] && i=$((i+1)) && [ $i -lt 3 ]; do :; done; echo i=$i");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "i=3\n");
    let (out, err, code) = run("i=0; while [ $i -lt 3 ] && false; do i=$((i+1)); done; echo i=$i");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "i=0\n");
}

/// The `until` twin: a cond-list condition is evaluated with the same
/// short-circuit rules, so the loop runs once (`break`) and reports 0.
/// Measured: `rc=0`.
#[test]
fn until_and_in_condition_runs_the_cond_list() {
    let (out, err, code) = run("until [ 1 ] && [ 1 ]; do echo hi; break; done; echo rc=$?");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "rc=0\n");
    let (out, err, code) = run("until [ 1 ] || [ 1 ]; do echo hi; break; done; echo rc=$?");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "rc=0\n");
}

/// `&&` / `||` in a loop *body* belong to the body: the loop is parsed whole
/// (master: `expected 'done'` rc 1). Measured against bash: `after` and the
/// per-iteration `a x b x`, on single- and multi-line bodies.
#[test]
fn and_in_loop_body_stays_inside_the_block() {
    let (out, err, code) = run("while true; do false && echo a; break; done; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
    let (out, err, code) = run("while true; do\n  false && echo a\n  break\ndone\necho after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
    let (out, err, code) = run("for w in a b; do echo $w && echo x; done; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "a\nx\nb\nx\nafter\n");
    let (out, err, code) = run("for w in a b; do\n  echo $w && echo x\ndone\necho after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "a\nx\nb\nx\nafter\n");
    let (out, err, code) = run("while [ 1 ] && [ 1 ]; do\n  echo hi\n  break\ndone\necho after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "hi\nafter\n");
}

// ——— Task #205: a loop control raised inside a condition is the loop's own ———
//
// The condition runs inside the loop's own level, so a `break`/`continue` in it
// (written directly, or in an `if` then-body, a `{ …; }` group, a `case` arm, a
// function called from the condition) is consumed by that loop: one level
// consumed, the remainder handed outward. An `if` is transparent, so its
// condition's control reaches the caller's loop whole. master dropped the signal
// (`?` on the cond-list run kept only the error), so every shape that must end a
// loop ran forever (rc 124 at the timeout) — they all run under [`run_bounded`].
// All expectations measured against `bash -c` (bash 5.3.9, 2026-10-13).

/// Row 1: a `break` in an `if` then-body inside a `while` condition ends the loop
/// before its body ever runs. master printed `body` forever, rc 124 at the timeout.
#[test]
fn break_in_an_if_condition_ends_the_while_loop() {
    let (out, err, code) =
        run_bounded("while if true; then break; fi; do echo body; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 2: the same `break` written in the loop body's `if` condition; the loop
/// ends before `echo body` runs (bash prints `end` only — not `body` `end`).
#[test]
fn break_in_an_if_condition_inside_a_while_body_ends_the_loop() {
    let (out, err, code) =
        run_bounded("while :; do if break; then :; fi; echo body; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 3: a `for` list is a loop level too, so a `break` in a body-`if` condition
/// ends it in the first iteration. master ran all three (`i=3`).
#[test]
fn break_in_an_if_condition_ends_the_for_list() {
    let (out, err, code) =
        run_bounded("i=0; for x in 1 2 3; do if break; then :; fi; i=$((i+1)); done; echo i=$i");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "i=0\n");
    assert_eq!(err, "");
}

/// Row 4: the bare spelling — `break` written straight in the `while` condition.
#[test]
fn bare_break_in_a_while_condition_ends_the_loop() {
    let (out, err, code) = run_bounded("while break; do echo body; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 6: `break 2` in a nested loop's condition consumes the inner loop and
/// hands `Break(1)` to the outer `for`, so both exit in the first iteration.
#[test]
fn break_2_in_a_while_condition_exits_both_loops() {
    let (out, err, code) =
        run_bounded("for a in 1 2 3; do while break 2; do :; done; echo inner; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 7: `continue 2` in a nested loop's condition advances the outer `for`
/// past the rest of its body, so `inner` never prints.
#[test]
fn continue_2_in_a_while_condition_advances_the_outer_for() {
    let (out, err, code) =
        run_bounded("for a in 1 2 3; do while continue 2; do :; done; echo inner; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 8: an `if` is transparent, so a `continue` raised in its condition reaches
/// the caller's `for` list and advances it. master printed `inner` three times.
#[test]
fn continue_in_an_if_condition_advances_the_for_list() {
    let (out, err, code) =
        run_bounded("for a in 1 2 3; do if continue; then :; fi; echo inner; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 9: an `elif` condition is a cond-list run like the `if` condition, so its
/// `break` ends the `for` list too.
#[test]
fn break_in_an_elif_condition_ends_the_for_list() {
    let (out, err, code) = run_bounded(
        "for a in 1 2 3; do if false; then :; elif break; then :; fi; echo inner; done; echo end",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 10: the `continue` twin in an `elif` condition.
#[test]
fn continue_in_an_elif_condition_advances_the_for_list() {
    let (out, err, code) = run_bounded(
        "for a in 1 2 3; do if false; then :; elif continue; then :; fi; echo inner; done; echo end",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 11: a `{ …; }` group in the condition is transparent, so its `break` is
/// the loop's; the `echo cond` after it never runs, and neither does the body.
#[test]
fn break_in_a_group_condition_ends_the_loop() {
    let (out, err, code) = run_bounded("while { break; echo cond; }; do echo body; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 12: a `case` arm in the condition, same rule.
#[test]
fn break_in_a_case_arm_condition_ends_the_loop() {
    let (out, err, code) =
        run_bounded("while case x in x) break; echo cond;; esac; do echo body; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// The `until` twin of row 1. master exited by accident (the condition's status
/// is 0, which is what ends an `until`), so this row agrees on master and pins
/// the rule, not the accident.
#[test]
fn break_in_an_if_condition_ends_the_until_loop() {
    let (out, err, code) =
        run_bounded("until if true; then break; fi; do echo body; done; echo end");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 14: the control crosses a nested `if` (both are transparent) and ends the
/// `for` list in its first iteration.
#[test]
fn break_in_a_nested_if_condition_ends_the_for_list() {
    let (out, err, code) = run_bounded(
        "for a in 1 2 3; do if true; then if break; then :; fi; fi; echo inner; done; echo end",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Depth 2: `break 2` in an inner `for` list's condition consumes the inner list
/// and exits the outer one.
#[test]
fn break_2_in_an_if_condition_exits_both_for_lists() {
    let (out, err, code) = run_bounded(
        "for a in 1 2 3; do for b in 1 2 3; do if break 2; then :; fi; echo inner; done; done; echo end",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// The `until` twin at depth 2: the condition's `break 2` exits the `until` and
/// the outer `for`.
#[test]
fn break_2_in_an_until_condition_exits_both_loops() {
    let (out, err, code) = run_bounded(
        "for a in 1 2 3; do until false; do if break 2; then :; fi; echo inner; done; echo inner; done; echo end",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Depth 2 with `continue 2`: the outer `for` advances, so neither `inner` runs.
#[test]
fn continue_2_in_an_if_condition_advances_the_outer_for() {
    let (out, err, code) = run_bounded(
        "for a in 1 2 3; do for b in 1 2 3; do if continue 2; then :; fi; echo inner; done; done; echo end",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "end\n");
    assert_eq!(err, "");
}

/// Row 15: a `return` raised in a loop condition propagates out of the loop, so
/// the function returns with the status the `return` set and `afterloop` never
/// prints. master looped forever.
#[test]
fn return_in_a_while_condition_leaves_the_function() {
    let (out, err, code) = run_bounded(
        "f() { while if true; then return 3; fi; do :; done; echo afterloop; }; f; echo rc=$?",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "rc=3\n");
    assert_eq!(err, "");
}

/// Row 16: the same `return` in an `if` condition leaves the function.
#[test]
fn return_in_an_if_condition_leaves_the_function() {
    let (out, err, code) =
        run_bounded("f() { if return 3; then :; fi; echo after; }; f; echo rc=$?");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "rc=3\n");
    assert_eq!(err, "");
}

/// Row 17: the `elif` twin.
#[test]
fn return_in_an_elif_condition_leaves_the_function() {
    let (out, err, code) = run_bounded(
        "f() { if false; then :; elif return 3; then :; fi; echo after; }; f; echo rc=$?",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "rc=3\n");
    assert_eq!(err, "");
}

/// Row 18: the body ran, and a later condition `break` ends the loop. Only the
/// stdout is pinned: the loop's own status diverges (bash `echo rc=$?` → 0,
/// fdshell → 1, measured 2026-10-13) because `break` does not zero `$?` — task
/// #262 owns that; task #164 owns the loop's condition-status rule.
#[test]
fn break_in_the_condition_after_the_body_ran_ends_the_loop() {
    let (out, err, code) = run_bounded(
        "i=0; while if [ $i -lt 2 ]; then :; else break; fi; do i=$((i+1)); echo body; done; echo i=$i",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "body\nbody\ni=2\n");
    assert_eq!(err, "");
}

/// Task #222: the `for … in` list runs the same splitter as a command
/// argument, so a list word splits **only on the bytes its expansions
/// produced** — a literal `IFS` byte in the list is content and glues
/// (`IFS=,; for w in x$(echo "b c"),y` is one iteration, exactly bash), and a
/// positional of an embedded `$@` starts a new iteration. bash 5.3.9 values;
/// the literal-word guard (`for w in a,b`) is task #133's rule.
#[test]
fn for_list_words_split_only_on_expansion_bytes() {
    let cases: &[(&str, &str)] = &[
        (
            r#"IFS=,; for w in x$(echo "b c"),y; do printf "[%s]\n" "$w"; done"#,
            "[xb c,y]\n",
        ),
        (
            r#"IFS=,; v="a b"; for w in x$v,y; do printf "[%s]\n" "$w"; done"#,
            "[xa b,y]\n",
        ),
        (
            r#"IFS=,; for w in $(printf "a,")$(printf ",b"); do printf "[%s]\n" "$w"; done"#,
            "[a]\n[]\n[b]\n",
        ),
        (
            r#"IFS=,; v="a,"; u="b"; for w in $v$u; do printf "[%s]\n" "$w"; done"#,
            "[a]\n[b]\n",
        ),
        (
            r#"IFS=" "; set -- "a b" c; for w in x$@y; do printf "[%s]\n" "$w"; done"#,
            "[xa]\n[b]\n[cy]\n",
        ),
        (
            r#"IFS=,; set -- "a b" c; for w in x"$@"y; do printf "[%s]\n" "$w"; done"#,
            "[xa b]\n[cy]\n",
        ),
        // Guards: a literal list word never splits (task #133), and the
        // whole-word `"$@"` is the positional words themselves.
        (
            r#"IFS=,; for w in a,b; do printf "[%s]\n" "$w"; done"#,
            "[a,b]\n",
        ),
        (
            r#"IFS=" "; set -- "a b" c; for w in "$@"; do printf "[%s]\n" "$w"; done"#,
            "[a b]\n[c]\n",
        ),
    ];
    for (script, want) in cases {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script:?} must succeed, stderr={err}");
        assert_eq!(&out, want, "{script:?}");
    }
}

/// Task #232: a loop written as a cond part (`true && while …`) and a loop
/// whose multi-line text ends with a gated operator run exactly like bash:
/// the segment scanner keeps the whole run, the cond layer splits the
/// operators around the block, and the loop's own `;` never splits the run.
#[test]
fn loops_as_cond_parts_run_like_bash() {
    let (out, err, code) = run("true && while :; do echo hi; break; done");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err}");
    let (out, err, code) = run("true && until false; do echo u; break; done && echo tail");
    assert_eq!((out.as_str(), code), ("u\ntail\n", 0), "stderr={err}");
    let (out, err, code) = run("while :; do\necho a\nbreak\ndone && echo x");
    assert_eq!((out.as_str(), code), ("a\nx\n", 0), "stderr={err}");
    let (out, err, code) = run("true && for i in 1 2; do echo $i; done; echo tail");
    assert_eq!((out.as_str(), code), ("1\n2\ntail\n", 0), "stderr={err}");
}

/// Review F1 (task #232): the mid-run span must not swallow a loop body. Body
/// runs are re-scanned with the mid-run rule, so a builtin `wait` written
/// after `&&` inside a body took a span as soon as it was not gated on the
/// shared scorer: the swallowed text held the `break`, the loop re-tested its
/// condition forever and the statements after the swallowed `;` were dropped
/// (rc 0, no diagnostic). Every row is bounded ([`run_bounded`]) so a
/// regression panics here with the script in its message instead of hanging
/// the suite. Measured against bash 5.3.9: `tail` (rc 0).
#[test]
fn mid_run_builtin_wait_keeps_the_loop_body() {
    for (script, want) in [
        ("while :; do true && wait; echo tail; break; done", "tail\n"),
        (
            "while :; do true && time wait; echo tail; break; done",
            "tail\n",
        ),
        (
            "while :; do echo a && wait; echo tail; break; done",
            "a\ntail\n",
        ),
        (
            "for i in 1; do false || wait; echo tail; break; done",
            "tail\n",
        ),
    ] {
        let (out, err, code) = run_bounded(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

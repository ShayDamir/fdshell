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

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). The pid-named temp files live inside this dir now. Removal is
/// `.ok()`-tolerant: teardown never panics, and it runs on a failing test's
/// unwind, which is exactly the test that used to leak.
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A pid + counter scratch dir `fdshell-loops-<tag>-<pid>-<counter>`,
/// removed when the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-loops-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// `source` shares the caller's loop depth, so `break 2` in a sourced file exits
/// both loops. Measured: only `end` printed (bash agrees).
#[test]
fn source_break_2_exits_both_loops() {
    let dir = scratch("source");
    let path = dir.join("src.sh");
    std::fs::write(&path, "break 2\n").unwrap();
    let script = format!(
        "for a in 1 2; do for b in 1 2; do source {}; echo inner; done; echo outer; done; echo end",
        path.to_str().unwrap()
    );
    let (out, code) = {
        let (out, _err, code) = run(&script);
        (out, code)
    };
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
///
/// Task #321 adds the pair that contributes no bytes (`for w in ~""`): the
/// mask has no bit for it, so the for-list words carry the tokenizer's span
/// list, and bash's `for w in ~""` iterates over `~`.
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
    // Task #321's zero-byte pairs: measured in bash 5.3.9, rc 0 in both shells
    // — `for w in ~""` iterates over `[~]`, `for w in ~''/x` over `[~/x]`, and
    // the empty pair at a word's end (`~x""`) keeps `[~x]`.
    for (script, want) in [
        (r#"for w in ~""; do printf "[%s]" "$w"; done"#, "[~]"),
        (r#"for w in ~''/x; do printf "[%s]" "$w"; done"#, "[~/x]"),
        (
            r#"for w in ~"" ~''/x ~x""; do printf "[%s]" "$w"; done"#,
            "[~][~/x][~x]",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, want, "{script}: bash iterates over {want}");
    }
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
    let dir = scratch("glob");
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
}

/// Task #220: the backtick fields reach the same `glob::expand` as the
/// `$( … )` fields, so the for-list surface honours the shell's glob options on
/// both forms. A non-matching pattern is dropped by `nullglob`, and `set -f`
/// (noglob) passes the pattern through verbatim without touching the
/// filesystem. Measured against `bash -c` 5.3.9 in a directory with `ab ac zz`:
/// both forms print nothing with `nullglob`, and `[*]` with `set -f`.
#[test]
fn for_list_backtick_fields_respect_glob_options() {
    let dir = scratch("globopt");
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

/// Row 18: the body ran, and a later condition `break` ends the loop. The
/// status is pinned too (task #262, measured 2026-10-19): the `break` is a
/// builtin whose own run succeeded, so `echo rc=$?` is 0 — the loop's
/// condition-status rule (bash also reports 0 when its condition ends false)
/// is task #164's, and is not exercised by this shape.
#[test]
fn break_in_the_condition_after_the_body_ran_ends_the_loop() {
    let (out, err, code) = run_bounded(
        "i=0; while if [ $i -lt 2 ]; then :; else break; fi; do i=$((i+1)); echo body; done; echo i=$i rc=$?",
    );
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "body\nbody\ni=2 rc=0\n");
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

// ——— Task #262: `break`/`continue` are builtins whose own status is 0 ———
//
// A loop that exits through a `break`/`continue` reports the *signal's* status,
// not the status of the last command that ran: bash's builtins return 0, so
// `false; break` leaves `$?` = 0. Every expectation below was measured against
// `bash -c` (bash 5.3.9, 2026-10-19), and the `echo rc=$?` each script ends in
// is the observation. master printed `rc=1` on every row that ends via a
// control. Loops that run forever when a raised control is dropped are bounded
// ([`run_bounded`]).

/// `break` after a failing command in a `for` list. Measured `rc=0` (master `rc=1`).
#[test]
fn break_after_a_failing_command_leaves_status_zero() {
    let (out, err, code) = run("for x in 1 2 3; do false; break; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// The `while` twin — `while :`, so a dropped `break` would run forever: bounded.
/// Measured `rc=0` (master `rc=1`).
#[test]
fn break_after_a_failing_command_in_a_while_leaves_status_zero() {
    let (out, err, code) = run_bounded("while true; do false; break; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// An `if` is transparent, so the `break` in its then-body is the loop's signal and
/// its 0 is the loop's status; `echo x` never runs. Measured `rc=0` (master `rc=1`).
#[test]
fn break_in_an_if_then_body_after_a_failure_leaves_status_zero() {
    let (out, err, code) =
        run("for x in 1 2 3; do if true; then false; break; fi; echo x; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// `continue` after a failing command: it is the last thing that runs in the
/// iteration, so its 0 is the loop's final status. Measured `rc=0` (master `rc=1`).
#[test]
fn continue_after_a_failing_command_leaves_status_zero() {
    let (out, err, code) = run("for x in 1 2; do false; continue; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// Same with a tail command: the `continue` skips it, so nothing overwrites the
/// signal's 0. Measured `rc=0`, no `tail` line (master `rc=1`).
#[test]
fn continue_leaves_status_zero_and_skips_the_rest_of_the_iteration() {
    let (out, err, code) = run("for x in 1 2 3; do false; continue; echo tail; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// `continue` in a `while` whose guard is endless and whose body breaks after a few
/// iterations: every iteration ends on the signal's 0 and the final `break` raises
/// its own 0. Measured `rc=0`; bounded, since the guard is `while :`.
#[test]
fn continue_in_a_bounded_while_leaves_status_zero() {
    let (out, err, code) = run_bounded(
        "i=0; while :; do false; i=$((i+1)); if (( i > 2 )); then break; fi; continue; done; echo rc=$?",
    );
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// Two signals in one iteration: the last one raised owns the status. Measured `rc=0`
/// (master `rc=1`).
#[test]
fn continue_then_break_in_one_iteration_leaves_status_zero() {
    let (out, err, code) = run("for x in 1 2 3; do false; continue; break; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// `break 2` crosses the inner level and hands the remainder outward: bash measures
/// `rc=0` on that propagation path too. Measured `rc=0`, no `inner` (master `rc=1`).
#[test]
fn break_2_after_a_failure_leaves_status_zero() {
    let (out, err, code) =
        run("for a in 1 2; do for b in 1 2; do false; break 2; done; echo inner; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// The `continue 2` twin of the propagation path. Measured `rc=0`, no `inner`
/// (master `rc=1`).
#[test]
fn continue_2_after_a_failure_leaves_status_zero() {
    let (out, err, code) = run(
        "for a in 1 2; do for b in 1 2; do false; continue 2; done; echo inner; done; echo rc=$?",
    );
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// A `{ …; }` group is a transparent level: the signal reaches the loop with its 0.
/// Measured `rc=0`, no `x` (master `rc=1`).
#[test]
fn break_in_a_group_after_a_failure_leaves_status_zero() {
    let (out, err, code) = run("for a in 1 2; do { false; break; }; echo x; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// A `case` arm is transparent too. Measured `rc=0`, no `x` (master `rc=1`).
#[test]
fn break_in_a_case_arm_after_a_failure_leaves_status_zero() {
    let (out, err, code) =
        run("for a in 1 2; do case $a in 1) false; break;; esac; echo x; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// The guard against zeroing unconditionally: a short-circuited `break` never runs,
/// so the failing command's status survives. Measured `rc=1` in both shells.
#[test]
fn short_circuited_break_keeps_the_failure_status() {
    let (out, err, code) = run("for x in 1; do false && break; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=1\n", 0), "stderr={err}");
    let (out, err, code) = run("for a in 1 2; do false && break; echo x; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("x\nx\nrc=0\n", 0), "stderr={err}");
}

/// The loop's 0 feeds the next cond part: with master the loop ended on 1 and the
/// `&&` tail never ran. Measured `after` + `rc=0`.
#[test]
fn continue_leaves_the_loop_status_for_the_next_cond_part() {
    let (out, err, code) = run("for x in 1 2; do false; continue; done && echo after; echo rc=$?");
    assert_eq!((out.as_str(), code), ("after\nrc=0\n", 0), "stderr={err}");
}

/// A `break` that leaves a function frame carries the same 0 — it is the function's
/// status. Only the status is pinned: fdshell keeps its accepted function-boundary
/// divergence (task #203) — the loop exits at the first call where bash warns and
/// keeps running. Measured `rc=0` in both shells (master `rc=1`).
#[test]
fn break_leaving_a_function_frame_leaves_status_zero() {
    let (out, err, code) = run("f() { false; break; }; for a in 1 2; do f; done; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=0\n", 0), "stderr={err}");
}

/// A single-line loop whose body statement ends with the plain `&` (task #354):
/// the `&` is a command-list terminator inside the block's text, so the shape
/// parses and the statement runs **inside the body** as a background job. Bounded
/// because a regression in the closer rule can make the loop unbounded.
#[test]
fn loop_body_statement_with_trailing_ampersand_runs() {
    let (out, err, rc) = run_bounded("for i in 1; do echo hi & done\nwait");
    assert_eq!((out.as_str(), rc), ("hi\n", 0), "stderr={err}");
    let (out, err, rc) = run_bounded("while :; do echo hi & break; done\nwait");
    assert_eq!((out.as_str(), rc), ("hi\n", 0), "stderr={err}");
    let (out, err, rc) = run_bounded("until false; do echo hi & break; done\nwait");
    assert_eq!((out.as_str(), rc), ("hi\n", 0), "stderr={err}");
}

/// A `for` **word list** takes no list terminator (POSIX #2.9.1: the `do`
/// follows the list), so `for i in 1 & do` stays a parse error — bash rc 2,
/// fdshell rc 1 (the documented rc-1-vs-2 family, README "Errors").
#[test]
fn ampersand_in_a_for_word_list_is_a_parse_error() {
    let (out, err, rc) = run("for i in 1 & do echo x; done");
    assert_eq!((out.as_str(), rc), ("", 1));
    assert!(err.contains("expected 'do'"), "{err}");
}

/// A `& ;` written **inside a loop body** runs nothing (task #355): the body
/// slice keeps the `;` glued behind the `&`, the body's own scan rejects the
/// split, and the loop never runs — fdshell rc 1, bash rc 2. Bounded like the
/// accepted shape above: a regression that lets the body run re-opens the
/// unbounded `while`/`until` hang.
#[test]
fn ampersand_then_semicolon_in_a_loop_body_runs_nothing() {
    for script in [
        "for i in 1; do echo a & ; done",
        "for i in 1 2; do echo a & ; done",
        "while :; do echo a & ; done",
        "until false; do echo a & ; done",
        "while :; do echo a & ; break; done",
        "until false; do echo a & ; break; done",
        "while : & ; do echo a; break; done",
        "until false & ; do echo a; break; done",
    ] {
        let (out, err, rc) = run_bounded(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 2: {err}");
    }
}

/// The `for` **word list** is not statement text: the body-slice rule of task
/// #355 does not reach it, and there the `&` is an ordinary word byte —
/// `for i in 1 & ; do echo x; done` runs two iterations (`1` and `&`),
/// pin-as-measured where bash stops with rc 2.
#[test]
fn ampersand_in_a_for_word_list_before_the_do_runs_the_amp_as_a_word() {
    let (out, _err, rc) = run("for i in 1 & ; do echo x; done");
    assert_eq!((out.as_str(), rc), ("x\nx\n", 0), "bash rc 2");
}

/// The split-time guard already covers the cond-pair and glued forms behind
/// the `&` in a body: `&|` and `& &` are invalid splits that run nothing
/// (bash rc 2, fdshell rc 1). The `&& ;` row is the **dangling-pair** family
/// of task #358: the tokenizer's `&&` is one word, not the plain `&`, the
/// `;` trims off the body slice, and the body's scan runs the dangling pair's
/// left side — pin-as-measured rc 0 where bash stops with rc 2.
#[test]
fn ampersand_pair_forms_in_a_loop_body_are_their_own_families() {
    for script in [
        "for i in 1; do echo a &| cat; done",
        "for i in 1; do echo a & & done",
    ] {
        let (out, _err, rc) = run_bounded(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 2");
    }
    let (out, _err, rc) = run_bounded("for i in 1; do echo a && ; done");
    assert_eq!(
        (out.as_str(), rc),
        ("a\n", 0),
        "bash rc 2: #358's dangling pair"
    );
}

/// A `&`-terminated loop **condition** is one background job per condition
/// evaluation (task #356): the flag is loop-level, so a three-iteration loop
/// records three jobs. The scratch file's line count **is** the iteration pin,
/// and the `$!` the script appends to it is the last job's pid — never read
/// `$!` out of stdout, which every background child shares (LESSONS). Bounded:
/// a `&`-terminated condition whose `break` is dropped is a fork bomb in bash
/// too.
#[test]
fn ampersand_terminated_condition_records_a_job_per_iteration() {
    let dir = scratch("cond-bg");
    let f = dir.join("n").to_str().unwrap().to_string();
    let script = format!(
        "i=0; while : & do i=$((i+1)); builtin echo $i >>{f}; [ $i -ge 3 ] && break; done\n\
         builtin echo \"last=$!\" >>{f}\nwait"
    );
    let (out, err, rc) = run_bounded(&script);
    assert_eq!((out.as_str(), rc), ("", 0), "stderr={err}");
    let text = std::fs::read_to_string(&f).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.get(..3).map(<[_]>::to_vec).unwrap_or_default(),
        ["1", "2", "3"],
        "one iteration per job"
    );
    let last = lines
        .get(3)
        .and_then(|l| l.strip_prefix("last="))
        .unwrap_or_default();
    let pid: i64 = last.parse().unwrap_or_default();
    assert!(pid > 0, "$! = {last:?}, not a job pid");
}

/// The status a `&`-terminated condition gates on is the **async 0** (task
/// #356, bash parity): `while false & do echo x; break; done` runs its body,
/// where master's foreground condition ran nothing. `until` sees the same 0 and
/// stops at once, so its body never runs — in bash either.
#[test]
fn ampersand_terminated_loop_condition_gates_on_the_async_status() {
    for (script, want) in [
        ("while : & do echo x; break; done", "x\n"),
        ("while false & do echo x; break; done", "x\n"),
        ("while true & do echo x; break; done", "x\n"),
        ("until false & do echo x; break; done", ""),
        ("until true & do echo x; break; done", ""),
        ("while : &\ndo echo x; break; done", "x\n"),
    ] {
        let (out, err, rc) = run_bounded(&format!("{script}\nwait"));
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: {err}");
    }
}

/// Task #292: a keyword block written as a pipeline **stage** is a command, not
/// the previous stage's arguments. Every loop keyword runs there with bash
/// (5.3.9) parity — including the glued `|` (task #236's pair rule sets the same
/// command position) and a multi-line stage, whose newlines are clause bytes.
/// The plan's `echo a | until ! :; do echo b; break; done` row is **not** pinned
/// here: `!` is unsupported at every position (a stage's `while ! :; …` and the
/// plain `! : ; echo tail` report the unresolved command on stderr the same way),
/// so the row's stdout parity is incidental — a failed `!` is a non-zero
/// condition, which is what `until` wants. `!` negation is task **#96**'s.
/// On `master` these lines printed the block's words as data
/// (`echo a | while :; do echo w; break; done` → `while :; do echo w; break; done`).
#[test]
fn keyword_block_as_a_pipeline_stage_runs_the_loop() {
    for (script, want) in [
        ("echo a | while :; do echo w; break; done", "w\n"),
        ("echo a | until :; do echo u; break; done", ""),
        ("echo a | for i in 1 2; do echo $i; done", "1\n2\n"),
        ("echo a | if :; then echo y; else echo n; fi", "y\n"),
        ("echo a | case x in x) echo c;; *) echo d;; esac", "c\n"),
        ("echo a|while :; do echo g; break; done", "g\n"),
        ("echo a | while :\n do\n echo m\n break\ndone", "m\n"),
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: {err}");
    }
}

/// The stage's block is a statement **of the fork**: its `break` ends only its
/// own loop (never the loop that encloses the pipeline), its `exit` ends only
/// the fork (the pipeline's status reaches the parent's `$?`), and a block stage
/// inside a loop body ends that loop normally. bash parity on every row.
#[test]
fn a_stage_blocks_loop_control_stays_inside_the_fork() {
    for (script, want) in [
        (
            "while :; do echo a | while :; do echo in; break; done; echo outer; break; done",
            "in\nouter\n",
        ),
        (
            "for i in 1; do echo a | while :; do break; done; echo tail; done",
            "tail\n",
        ),
        (
            "while :; do echo a | while :; do echo b; break; done; echo tail; break; done",
            "b\ntail\n",
        ),
        (
            "echo a | while :; do exit 3; done; echo tail rc=$?",
            "tail rc=3\n",
        ),
    ] {
        let (out, err, rc) = run_bounded(script);
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: {err}");
    }
}

/// The stage's loop reads the pipe: the first line arrives. The row keeps
/// fdshell's pre-existing `read` divergence (the body runs once and the stage's
/// status is the failed `read`, so the pipeline reports rc 1 where bash reports
/// 0) — the `read` class owns that, it is measured for the `< file` twin on
/// `master` too, and #292 changes only the block's position.
#[test]
fn a_stage_block_reads_the_pipe_with_the_read_divergence() {
    let (out, err, rc) = run("echo a | while read l; do echo r-$l; done");
    assert_eq!((out.as_str(), rc), ("r-a\n", 1), "{err}: bash rc 0");
}

/// The POSIX builtin `wait` behind a `|` opens **no** block span (the shared
/// scorer's gate: no pattern keyword behind the word), so it is an ordinary
/// command stage — and the fork has no `wait` to dispatch: it reports the
/// unresolved command on stderr while the pipeline's own status stays 0. The
/// stage-child gap of task **#381** (the `cd` twin is pinned there).
#[test]
fn the_builtin_wait_behind_a_pipe_is_a_command_stage() {
    let (out, err, rc) = run("echo a | wait; echo tail");
    assert_eq!((out.as_str(), rc), ("tail\n", 0), "{err}");
    assert!(
        err.contains("failed to resolve command path"),
        "bash runs the builtin silently: {err}"
    );
}

/// A control builtin behind the `|` is an ordinary **stage**: the fork has no
/// `break`/`continue` to run (they are not loop keywords of a stage), so it
/// reports the unresolved command on stderr and the statement behind the
/// pipeline runs — the visible pair (stdout, rc 0) is bash's, only the wording of
/// the two diagnostics differs. The stage-child gap of task **#381**.
#[test]
fn a_control_builtin_behind_a_pipe_is_a_command_stage() {
    for script in ["echo a | break; echo tail", "echo a | continue; echo tail"] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("tail\n", 0), "{script}: {err}");
        assert!(
            err.contains("failed to resolve command path"),
            "{script}: bash prints its own diagnostic: {err}"
        );
    }
}

/// A block stage's status is its loop's own last status (the `break` is the last
/// command, rc 0 — bash parity), and a `&` on the pipeline puts the **whole**
/// pipeline, block stage included, into the background job: the parent prints
/// `started`, the job's stage prints `b`, `wait` collects it, bash parity
/// (`started\nb\ndone`, 5.3.9).
///
/// The `started`/`b` pair is written by **two processes** to one shared stdout,
/// so the order they land in is a race in every shell — measured 2/40 runs of
/// `b` first on the debug binary, and bash's own ordering comes from the same
/// race. The pin is therefore the **set** of the two racing lines plus the
/// `done` that `wait` puts last: the background job is reaped before it.
#[test]
fn a_block_stage_status_and_a_trailing_ampersand_are_bash_parity() {
    let (out, err, rc) = run("echo a | while :; do false; break; done; echo rc=$?");
    assert_eq!((out.as_str(), rc), ("rc=0\n", 0), "{err}");
    let (out, err, rc) =
        run_bounded("echo a | while :; do echo b; break; done & echo started; wait; echo done");
    let mut lines: Vec<&str> = out.lines().collect();
    assert_eq!((lines.len(), rc), (3, 0), "stdout={out} stderr={err}");
    assert_eq!(
        lines.pop(),
        Some("done"),
        "`wait` reaps the job before `done`: {out}"
    );
    lines.sort_unstable();
    assert_eq!(
        lines,
        ["b", "started"],
        "the parent's line and the job stage's line are a race: {out}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// The arithmetic `for ((init;cond;step)) ; do … ; done` (task #293).
//
// Every pin below was measured against bash 5.3.9 (`bash -c`) and
// `target/debug/fdshell -c` on 2026-10-22; the doc comment states both sides.
// ─────────────────────────────────────────────────────────────────────────

/// The three clauses in bash's order — init once, condition before each body,
/// step after it. bash `0 1 2` rc 0, fdshell the same.
#[test]
fn c_style_for_runs_its_three_clauses() {
    let (out, err, rc) = run("for ((i=0;i<3;i++)); do echo $i; done; echo rc=$?");
    assert_eq!((out.as_str(), rc), ("0\n1\n2\nrc=0\n", 0), "{err}");
    for spelling in [
        "for (( i=0 ; i<3 ; i++ )) ; do echo $i; done",
        "for ((i=0;i<3;i++)) do echo $i; done",
        "for ((i=0;i<3;i++)); do echo $i; done",
    ] {
        let (out, err, rc) = run(spelling);
        assert_eq!((out.as_str(), rc), ("0\n1\n2\n", 0), "{spelling}: {err}");
    }
    // A `do` glued to the region and a `do` after a newline are both accepted
    // (bash: `0 1` and `0 1 2`).
    let (out, err, rc) = run("for ((i=0;i<2;i++))do echo $i; done");
    assert_eq!((out.as_str(), rc), ("0\n1\n", 0), "{err}");
    let (out, err, rc) = run("for ((i=0;i<3;i++))\ndo echo $i\ndone");
    assert_eq!((out.as_str(), rc), ("0\n1\n2\n", 0), "{err}");
}

/// The clause is an arithmetic expression, so every operator `arith` knows is
/// clause data: compound assignment, pre-increment, plain assignment, a `$(…)`
/// (whose inner `;` is not a clause cut), a parenthesised init, a shift, and an
/// empty init with an unset variable reading 0. bash and fdshell agree on all
/// of them (measured).
#[test]
fn c_style_for_clause_shapes_match_bash() {
    for (script, want) in [
        ("for ((i=0;i<3;i+=2)); do echo $i; done", "0\n2\n"),
        ("for ((i=0;i<3;++i)); do echo $i; done", "0\n1\n2\n"),
        ("for ((i=0;i<3;i=i+1)); do echo $i; done", "0\n1\n2\n"),
        ("for ((i=0;i<$(echo 3);i++)); do echo $i; done", "0\n1\n2\n"),
        ("for ((i=(0);i<2;i++)); do echo $i; done", "0\n1\n"),
        ("for ((i=0;i<(1<<2);i++)); do echo $i; done", "0\n1\n2\n3\n"),
        // An empty init leaves `i` unset, so the first body prints an empty
        // line and the step's `i++` starts from 0 (bash's `~1~2`).
        ("for ((1;i<3;i++)); do echo $i; done", "\n1\n2\n"),
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: {err}");
    }
}

/// The comma operator lives inside one clause: bash's `for ((i=1,j=2;i<3;i++))`
/// binds both vars and steps only `i`, so the pairs are `1 2` and `2 2` — the
/// measured second line, not `2 3`.
#[test]
fn c_style_for_comma_init_binds_both_vars() {
    let (out, err, rc) = run("for ((i=1,j=2;i<3;i++)); do echo \"$i $j\"; done");
    assert_eq!((out.as_str(), rc), ("1 2\n2 2\n", 0), "{err}");
}

/// An empty condition is true and an empty step is a no-op, so the loop is
/// infinite until its body breaks: bash prints `x`, and so does the empty
/// region's spaced spelling. Both pins are bounded (a dropped `break` here is
/// the hang this task's risk section names).
#[test]
fn c_style_for_empty_clauses_need_a_break() {
    for script in [
        "for ((;;)); do echo x; break; done",
        "for (( ; ; )) ; do echo x; break; done",
    ] {
        let (out, err, rc) = run_bounded(script);
        assert_eq!((out.as_str(), rc), ("x\n", 0), "{script}: {err}");
    }
    // `for ((i=0;;))` prints its own init value, `for ((;i<2;))` its body word.
    let (out, err, rc) = run_bounded("for ((i=0;;)); do echo $i; break; done");
    assert_eq!((out.as_str(), rc), ("0\n", 0), "{err}");
    let (out, err, rc) = run_bounded("for ((;i<2;)); do echo a; break; done");
    assert_eq!((out.as_str(), rc), ("a\n", 0), "{err}");
}

/// The status rules of task #164, in bash's measured triple: a body that never
/// ran leaves 0 (after a `false`), the body's last command is the status, and a
/// `break` leaves its own 0.
#[test]
fn c_style_for_status_rules_match_bash() {
    let (out, err, rc) = run("false; for ((i=0;i<0;i++)); do :; done; echo $?");
    assert_eq!((out.as_str(), rc), ("0\n", 0), "{err}");
    let (out, err, rc) = run("for ((i=0;i<3;i++)); do false; done; echo $?");
    assert_eq!((out.as_str(), rc), ("1\n", 0), "{err}");
    let (out, err, rc) = run("for ((i=0;i<2;i++)); do echo $i; break; done; echo $?");
    assert_eq!((out.as_str(), rc), ("0\n0\n", 0), "{err}");
    // The loop's own status is the last thing it reported, so a statement after
    // it is the shell's rc (`echo rc=$?` after the loop reads 0, bash parity).
    let (out, err, rc) = run("for ((i=0;i<0;i++)); do echo body; done; echo rc=$?");
    assert_eq!((out.as_str(), rc), ("rc=0\n", 0), "{err}");
}

/// The clauses run **in-process**, like the `((expr))` command, so the loop var
/// is an ordinary shell variable that survives the loop (bash: `3 2 1` then
/// `i=0`, the value the last step left behind).
#[test]
fn c_style_for_loop_var_survives_the_loop() {
    let (out, err, rc) = run("for ((i=3;i>0;i--)); do echo $i; done; echo i=$i");
    assert_eq!((out.as_str(), rc), ("3\n2\n1\ni=0\n", 0), "{err}");
}

/// Body control flow: `break` skips the step, `continue` runs it and re-tests
/// the condition, and a nested `break 2` leaves both loops (bash's three
/// measured shapes). Every pin can hang when the fix regresses, so all run
/// under the 5 s bound.
#[test]
fn c_style_for_body_control_flow() {
    let (out, err, rc) = run_bounded("for ((i=0;i<2;i++)); do echo a; break; done; echo rc=$?");
    assert_eq!((out.as_str(), rc), ("a\nrc=0\n", 0), "{err}");
    let (out, err, rc) =
        run_bounded("for ((i=0;i<2;i++)); do continue; echo never; done; echo done");
    assert_eq!((out.as_str(), rc), ("done\n", 0), "{err}");
    let (out, err, rc) = run_bounded(
        "for ((i=0;i<2;i++)); do for ((j=0;j<2;j++)); do break 2; done; done; echo rc=$?",
    );
    assert_eq!((out.as_str(), rc), ("rc=0\n", 0), "{err}");
    let (out, err, rc) = run_bounded(
        "for ((i=0;i<2;i++)); do for ((j=0;j<2;j++)); do [ \"$j\" = 1 ] && break 2; echo \"$i$j\"; done; done",
    );
    assert_eq!((out.as_str(), rc), ("00\n", 0), "{err}");
}

/// The arithmetic `for` is a keyword block, so it is a pipeline **stage** in
/// both directions (task #292's block stage). bash `0 1` for each; the fork of
/// a stage means the loop var dies with it, as bash's subshell stage does.
#[test]
fn c_style_for_is_a_pipeline_stage() {
    let (out, err, rc) = run("for ((i=0;i<2;i++)) do echo $i; done | cat");
    assert_eq!((out.as_str(), rc), ("0\n1\n", 0), "{err}");
    let (out, err, rc) = run("echo a | for ((i=0;i<2;i++)); do echo $i; done");
    assert_eq!((out.as_str(), rc), ("0\n1\n", 0), "{err}");
}

/// `&&`/`||` hand their part to this parser (task #232's mid-run span):
/// `true && for ((…)) …` and `false || for ((…)) …` both run the loop. bash's
/// `false || …` prints `y` twice, one per iteration — the same, measured.
#[test]
fn c_style_for_midrun_after_and_or() {
    let (out, err, rc) = run("true && for ((i=0;i<2;i++)); do echo $i; done");
    assert_eq!((out.as_str(), rc), ("0\n1\n", 0), "{err}");
    let (out, err, rc) = run("false || for ((i=0;i<2;i++)); do echo y; done");
    assert_eq!((out.as_str(), rc), ("y\ny\n", 0), "{err}");
}

/// A here-doc written in the body is re-read **per iteration** (bash prints the
/// body twice), in the single-line spelling (the bodies sit past `done`, so
/// `part_text` appends them) and in the multi-line one (the verbatim text).
#[test]
fn c_style_for_heredoc_body_repeats_per_iteration() {
    let (out, err, rc) = run("for ((i=0;i<2;i++)); do cat <<E; done\nbody\nE");
    assert_eq!((out.as_str(), rc), ("body\nbody\n", 0), "{err}");
    let (out, err, rc) = run("for ((i=0;i<2;i++)); do\ncat <<E\nbody\nE\ndone");
    assert_eq!((out.as_str(), rc), ("body\nbody\n", 0), "{err}");
}

/// In a `{ …; }` group with a redirection on the group, the loop's output is
/// what lands in the file (bash: 2 lines). The redirection written after the
/// block's own `done` is the separate, pre-existing gap task **#298** documents
/// (the word form drops it identically, so both `for` forms share it).
#[test]
fn c_style_for_in_a_group_with_a_redirect() {
    let dir = scratch("cstyle-group");
    let file = dir.join("out").to_str().unwrap().to_string();
    let (out, err, rc) = run(&format!(
        "{{ for ((i=0;i<2;i++)); do echo $i; done; }} > {file}; wc -l < {file}"
    ));
    assert_eq!((out.as_str(), rc), ("2\n", 0), "{err}");
}

/// Everything bash rejects, fdshell rejects too (bash rc 2, fdshell rc 1 — the
/// repo-wide parse-error convention): a clause count that is not three, a word
/// between `))` and `do`, a redirection there, the `;;`/`; ;` spellings, a pipe,
/// and an empty `do` body. Nothing runs, so stdout stays empty.
#[test]
fn c_style_for_rejects_bash_rejects() {
    for script in [
        "for ((i=0))",
        "for ((i=0;i<2))",
        "for ((i=0;))",
        "for ((i=0;i<2;i++;))",
        "for (( in 1 2 ))",
        "for ((i=0\ni<2\ni++)); do echo a; break; done",
        "for ((i=0;i<2;i++))x; do :; done",
        "for ((i=0;i<2;i++)) 2>/dev/null; do :; done",
        "for ((i=0;i<2;i++)) > /dev/null; do :; done",
        "for ((i=0;i<2;i++)) ;; do :; done",
        "for ((i=0;i<2;i++)) ; ; do :; done",
        "for ((i=0;i<2;i++)) | cat",
        "for ((i=0;i<2;i++)); do; done",
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: {err}");
        assert!(err.contains("parse error"), "{script}: {err}");
    }
}

/// `<<` inside the region is the shift operator, so it claims no here-doc body:
/// the region is opaque to the heredoc layer (the statement path detects the
/// arithmetic `for` before `heredoc::layout` counts its specs). bash prints the
/// body once per iteration; so does fdshell.
#[test]
fn c_style_for_shift_in_the_region_is_not_a_heredoc() {
    let (out, err, rc) = run("for ((i=0;i<(1<<2);i++)); do cat <<B; done\nbody\nB");
    assert_eq!((out.as_str(), rc), ("body\nbody\nbody\nbody\n", 0), "{err}");
}

/// The heredoc-spec asymmetry the plan predicted for the **stage** path (the
/// stage's spec share is counted before the arithmetic `for` is detected, so a
/// `<<` hidden in the region could desync it) measured as **bash parity**: the
/// stage prints the body once per iteration, exactly like bash and like the
/// statement-path pin above. Pinned as the parity it is, so a future spec-cursor
/// change shows up here.
#[test]
fn c_style_for_a_stage_with_a_shift_and_a_heredoc_is_bash_parity() {
    let (out, err, rc) = run("echo a | for ((i=0;i<(1<<2);i++)); do cat <<B; done\nbody\nB");
    assert_eq!((out.as_str(), rc), ("body\nbody\nbody\nbody\n", 0), "{err}");
}

/// Two forms bash accepts stay open divergences, each with its own task, and
/// both fail loudly rather than silently: the `{ …; }` body (task **#394**,
/// fdshell: a parse error, rc 1) and the glued `for((…))` (task **#396**, not a
/// keyword word at all, so `do`/`done` run as commands).
#[test]
fn c_style_for_the_glued_and_brace_body_forms_stay_divergences() {
    let (out, err, rc) = run("for ((i=0;i<2;i++)) { echo $i; }");
    assert_eq!((out.as_str(), rc), ("", 1), "task #394: {err}");
    assert!(err.contains("parse error"), "task #394: {err}");
    let (out, err, rc) = run("for((i=0;i<3;i++));do echo $i;done");
    assert_eq!(out, "", "task #396: the glued word is no keyword: {out}");
    assert_eq!(
        rc, 1,
        "task #396: bash rc 0, fdshell runs `do`/`done`: {err}"
    );
    assert!(
        err.contains("failed to resolve command path"),
        "task #396: {err}"
    );
}

/// `set -u` does not stop the clauses (task **#170** owns nounset in
/// arithmetic): the loop runs `0 1` at rc 0, bash parity, because the init
/// clause binds the loop var before the condition reads it.
#[test]
fn c_style_for_ignores_set_u_like_arith() {
    let (out, err, rc) = run("set -u; for ((i=0;i<2;i++)) do echo $i; done; echo rc=$?");
    assert_eq!((out.as_str(), rc), ("0\n1\nrc=0\n", 0), "{err}");
}

/// A clause that is not a valid arithmetic expression stops the loop with rc 1
/// and prints no body, as bash's does: the init's `$(false)` splices to an empty
/// operand, and `i<` has no right operand. fdshell's message is its own
/// (arithmetic syntax error), and the statement aborts — the `((expr))`
/// command's rule, not bash's "report and continue".
#[test]
fn c_style_for_arith_clause_error_stops_the_loop_like_bash() {
    for script in [
        "for ((i=$(false);i<1;i++)); do echo a; done",
        "for ((i=0;i<;i++)); do echo a; done",
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 1");
        assert!(
            err.contains("arithmetic expression has a syntax error"),
            "{script}: {err}"
        );
    }
}

/// A `#` inside a clause is data to the byte scan, so the region stays closed
/// across the comment and the newline; the step's own arithmetic then fails
/// exactly where bash's does — the body prints once, the step errors, rc 1
/// (bash 5.3.9: `0` then `((: i++ # c~: arithmetic syntax error`).
#[test]
fn c_style_for_comment_in_a_clause_is_bash_parity() {
    let (out, err, rc) = run("for ((i=0;i<2;i++ # c\n)) ; do echo $i; done");
    assert_eq!((out.as_str(), rc), ("0\n", 1), "{err}");
    let (out, err, rc) = run_bounded("for ((i=0;i<2;i++ # c\n)) ; do echo $i; break; done");
    assert_eq!((out.as_str(), rc), ("0\n", 0), "{err}");
}

/// The nesting forms: a command substitution captures the loop's stdout, `time`
/// reports it, and an `if` condition, a `case` arm and a `while` body each run
/// it as a statement (bash's measured outputs).
#[test]
fn c_style_for_nesting_forms_match_bash() {
    let (out, err, rc) = run("x=$(for ((i=0;i<2;i++)); do echo $i; done); echo \"cap=$x\"");
    assert_eq!((out.as_str(), rc), ("cap=0\n1\n", 0), "{err}");
    let (out, err, rc) = run("if for ((i=0;i<2;i++)); do echo $i; done; then echo ok; fi");
    assert_eq!((out.as_str(), rc), ("0\n1\nok\n", 0), "{err}");
    let (out, err, rc) = run("case x in x) for ((i=0;i<2;i++)); do echo $i; done;; esac");
    assert_eq!((out.as_str(), rc), ("0\n1\n", 0), "{err}");
    let (out, err, rc) =
        run_bounded("while false; do :; done; for ((i=0;i<2;i++)); do echo $i; done");
    assert_eq!((out.as_str(), rc), ("0\n1\n", 0), "{err}");
    let (out, err, rc) = run("time for ((i=0;i<2;i++)); do echo $i; done");
    assert_eq!((out.as_str(), rc), ("0\n1\n", 0), "{err}");
}

/// A function body runs the loop and a `return` in it leaves the function with
/// the returned status (bash: `0` then `rc=3`, the body's `echo after` never
/// printed).
#[test]
fn c_style_for_in_a_function_body_returns() {
    let (out, err, rc) =
        run("f() { for ((i=0;i<2;i++)); do echo $i; return 3; done; echo after; }; f; echo rc=$?");
    assert_eq!((out.as_str(), rc), ("0\nrc=3\n", 0), "{err}");
}

/// errexit stops the shell on the body's failure, as bash's does (rc 1, no
/// output); the arithmetic `for` propagates the `Exit` control like every other
/// loop.
#[test]
fn c_style_for_under_errexit_exits_the_shell() {
    let (out, err, rc) = run("set -e; for ((i=0;i<2;i++)); do false; done; echo tail");
    assert_eq!((out.as_str(), rc), ("", 1), "{err}");
}

/// A `${…}` expansion inside a clause is the pre-existing arithmetic gap of task
/// **#397** (`arith` expands only `$(…)` and `$((…))` in a clause, so a braced
/// expansion is a syntax error on **every** arithmetic surface — the `((expr))`
/// command and `$((…))` fail identically). bash runs the loop on `${n:-2}`.
#[test]
fn c_style_for_braced_expansion_in_a_clause_is_the_arith_gap() {
    let (out, err, rc) = run("n=2; for ((i=0;i<${n:-2};i++)); do echo $i; done");
    assert_eq!((out.as_str(), rc), ("", 1), "task #397: bash prints 0 1");
    assert!(
        err.contains("arithmetic expression has a syntax error"),
        "task #397: {err}"
    );
}

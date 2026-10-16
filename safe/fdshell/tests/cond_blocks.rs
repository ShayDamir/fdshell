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

/// Run with the child's cwd set to a fresh scratch dir, for the shapes that
/// write a file (the harness itself runs from the repo root).
fn run_in(dir: &std::path::Path, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .current_dir(dir)
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

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fdshell-cond_blocks-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Task #137: a cond-list operator in an `if` condition is the block's own cond
/// list. Measured against `bash -c` (5.3.9): `[ 0 ]` is a true test (a non-empty
/// word), so the first shape takes `then`; the second short-circuits to `else`.
#[test]
fn and_in_an_if_condition_runs_the_blocks_own_cond_list() {
    let (out, err, code) = run("if [ 1 ] && [ 0 ]; then echo t; else echo f; fi; echo rc=$?");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "t\nrc=0\n");
    let (out, err, code) = run("if false && true; then echo t; else echo f; fi; echo rc=$?");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "f\nrc=0\n");
}

/// `&&` / `||` inside a block *body* belong to the body: the block is never
/// split in half, so the `if`/`fi` pair stays intact. Measured: bash `after` rc 0
/// on every shape, master `missing 'fi'` / `expected 'done'` rc 1.
#[test]
fn and_in_a_block_body_stays_inside_the_block() {
    let (out, err, code) = run("if true; then false && echo a; fi; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
    let (out, err, code) =
        run("if true; then\n  if false; then echo inner; fi && echo tail\nfi\necho after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "tail\nafter\n");
    let (out, err, code) =
        run("while true; do\n  if true; then false; fi && echo tail\n  break\ndone\necho after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
}

/// A `case` arm body keeps its operator: master reported `missing 'esac'` rc 1.
/// Measured: bash prints the arm's output and `after`, rc 0.
#[test]
fn and_in_a_case_arm_body_stays_inside_the_block() {
    let (out, err, code) = run("case x in a) false && echo b;; esac; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
    let (out, err, code) = run("case x in x) echo x && echo more;; esac; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "x\nmore\nafter\n");
}

/// Groups as cond-list parts *inside* a block condition: the group rule and the
/// block rule coexist (the split inside the group body is the group's own, and
/// the group's `&&` after `}` is the outer list's). Measured: bash `hi` / `d`.
#[test]
fn group_cond_parts_inside_a_block_condition() {
    let (out, err, code) = run("while { true; } && { true; }; do echo hi; break; done");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "hi\n");
    let (out, err, code) = run("{ while false; do b && c; done; } && echo d");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "d\n");
}

/// A `time [-p]` prefix is transparent: the block keyword behind it opens the
/// block, and the report still prints. Measured: stdout `hi`, rc 0 (the report
/// goes to stderr in bash and here).
#[test]
fn time_prefixed_block_with_and_in_its_condition() {
    let (out, _err, code) = run("time while [ 1 ] && [ 1 ]; do echo hi; break; done");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\n");
}

/// A nested keyword block in a condition is opaque to the outer scan; the
/// shape that worked before (no operator) is untouched. Measured: bash `after`.
#[test]
fn nested_block_in_a_condition_is_opaque() {
    let (out, err, code) =
        run("while if true; then false; fi && true; do echo hi; break; done; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
    let (out, err, code) = run("while if true; then false; fi; do echo hi; done; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
}

/// The event-case `wait` block (fdshell-only syntax, so bash is a syntax error
/// here): an arm body's `&&` is the arm's own cond list, and a failing first
/// part short-circuits the arm. Measured: `f` `tail` `after`, then `after`.
#[test]
fn and_in_a_wait_arm_body_stays_inside_the_block() {
    let (out, err, code) =
        run("builtin true &>&j; wait\n finished %&j)\n echo f && echo tail ;;\ndone; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "f\ntail\nafter\n");
    let (out, err, code) =
        run("builtin true &>&j; wait\n finished %&j)\n false && echo tail ;;\ndone; echo after");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "after\n");
}

/// The fix touches no errexit code: the list-level rules (a skipped `&&` tail
/// never triggers errexit; a failing block condition under `set -e` exits) are
/// exactly master's. Measured against bash: every shape prints `after`, rc 0.
/// (`set -e; while true; do false && echo a; break; done` is deliberately absent:
/// a `break` carrying a failing body part's status exits under `set -e` on
/// master and here alike — bash prints `after` — the loop-status divergence is
/// task #164's, not this change's.)
#[test]
fn errexit_behavior_around_blocks_is_unchanged() {
    for script in [
        "set -e; false && echo x; echo after",
        "set -e; while false; do echo hi; done; echo after",
        "set -e; if false && true; then echo t; fi; echo after",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert_eq!(out, "after\n", "{script}");
    }
}

/// Accepted divergence, documented in README: `for i in a && b` is a bash
/// syntax error (rc 2); here the `&&` is a literal word of the `in` list and the
/// loop runs over `a`, `&&`, `b` (the `break` ends it in the first iteration).
#[test]
fn for_list_with_and_is_an_accepted_divergence() {
    let (out, _err, code) = run("for i in a && b; do echo hi; break; done; echo after");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\nafter\n");
}

/// Known gap, task #234: a block whose text carries a `<<` operator keeps the
/// old split (its condition bodies travel inside the span, and the pinned
/// `tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part`
/// depends on it), so `&&`/`||` in such a block stay broken — bash rejects this
/// shape with rc 2, fdshell reports a parse error rc 1.
#[test]
fn and_in_a_block_that_carries_a_heredoc_still_splits() {
    let (_out, _err, code) = run("while cat <<X; do a && b; X\ndone\necho after");
    assert_eq!(code, 1);
    let (_out, _err, code) = run("if cat <<X; then a && b; X\nfi\necho after");
    assert_eq!(code, 1);
}

/// Task #205: the shape is parsed **and** the `break` works — a loop control raised
/// in a loop condition is the loop's own signal, so it ends the loop in the first
/// condition test. Measured against `bash -c` (5.3.9): `i=0`, rc 0 (master dropped
/// the control and ran every iteration, `i=3`).
#[test]
fn break_in_an_and_condition_ends_the_loop() {
    let (out, _err, code) = run("i=0; while [ $i -lt 3 ] && break; do i=$((i+1)); done; echo i=$i");
    assert_eq!(code, 0);
    assert_eq!(out, "i=0\n");
}

/// Task #232 (was the pinned gap of task #137): an operator *adjacent* to a
/// block makes the block a cond part — the segment scanner keeps the whole
/// run (the block's own `;` are not separators), the cond layer splits the
/// operator, and the block runs (or is skipped) as a part. Measured against
/// bash 5.3.9: `hi` rc 0.
#[test]
fn operator_adjacent_to_a_block_makes_the_block_a_cond_part() {
    let (out, err, code) = run("true && while :; do echo hi; break; done");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err}");
}

/// The short-circuit applies to the block part: `false &&` skips it (rc 1,
/// the last-run part's status, exactly like the group twin), `false ||` runs
/// it. Measured: bash (nothing) rc 1 / `hi` rc 0.
#[test]
fn midrun_block_respects_the_cond_short_circuit() {
    let (out, err, code) = run("false && while :; do echo hi; break; done");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}");
    let (out, err, code) = run("false || while :; do echo hi; break; done");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err}");
}

/// Every block opener joins the run mid-cond — `if`, `for`, `case`, `until`,
/// `while`, and a `name() { … }` definition (the definition's trailing
/// `&& f` runs the defined function). All rows measured against bash.
#[test]
fn every_opener_joins_the_run_mid_cond() {
    for (script, want) in [
        ("echo a && if true; then echo b; fi", "a\nb\n"),
        ("true && for i in 1 2; do echo $i; done", "1\n2\n"),
        ("true && case x in x) echo cx;; esac", "cx\n"),
        ("true && until :; do echo u; break; done", ""),
        ("true && f() { echo F; } && f", "F\n"),
        ("x=1 && while :; do echo b; break; done", "b\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// `time [-p]` is transparent before a mid-run block, all three spellings
/// (LESSONS: a statement-prefix keyword must be transparent in *every*
/// first-word scorer): the block runs, the report goes to stderr, rc 0.
#[test]
fn time_prefix_is_transparent_before_a_midrun_block() {
    for prefix in ["time ", "time -p ", "time time -p "] {
        let script = format!("{prefix}while :; do echo hi; break; done");
        let (out, _err, code) = run(&script);
        assert_eq!((out.as_str(), code), ("hi\n", 0), "{script}");
        let script = format!("true && {prefix}while :; do echo hi; break; done");
        let (out, _err, code) = run(&script);
        assert_eq!((out.as_str(), code), ("hi\n", 0), "{script}");
    }
}

/// An operator written after a mid-run block (on the same run) is the cond
/// list's own: the mid-run block runs and the tail is gated by its status.
#[test]
fn trailing_operator_after_a_midrun_block_splits_the_list() {
    let (out, err, code) = run("true && while :; do echo hi; break; done && echo y");
    assert_eq!((out.as_str(), code), ("hi\ny\n", 0), "stderr={err}");
    let (out, err, code) = run("false && while :; do echo hi; break; done && echo y");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}");
}

/// A mid-run block that spans physical lines (the `do` on the opener's line,
/// the body on the lines below) stays one run: the block runs as a cond part
/// and the statement ends where the block's closing keyword's line does.
#[test]
fn midrun_block_spans_physical_lines() {
    let (out, err, code) = run("true && while :; do\necho hi\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err}");
    let (out, err, code) = run("true && if true; then\necho b\nfi && echo y");
    assert_eq!((out.as_str(), code), ("b\ny\n", 0), "stderr={err}");
}

/// A mid-run block inside a block body: the body is re-scanned, the mid-run
/// rule applies there too, and the outer block stays intact.
#[test]
fn midrun_block_inside_a_block_body() {
    let (out, err, code) = run("if true; then true && while :; do echo hi; break; done; fi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err}");
    let (out, err, code) =
        run("case q in q) true && for i in 1; do echo c$i; done ;; esac; echo tail");
    assert_eq!((out.as_str(), code), ("c1\ntail\n", 0), "stderr={err}");
}

/// Task #232 gap 2: a trailing operator on a multi-line block (the closing
/// keyword at the start of its physical line) is a cond part gated by the
/// block's status — the run reset starts a new word, the closer scores, and
/// the depth returns to 0. The indented variant worked before; both stay.
/// Rows measured against bash; `if false; then … fi` exits 0 without an
/// `else`, so its tail runs (bash agrees).
#[test]
fn trailing_operator_after_a_multi_line_block_is_gated() {
    let (out, err, code) = run("while :; do\necho a\nbreak\ndone && echo x");
    assert_eq!((out.as_str(), code), ("a\nx\n", 0), "stderr={err}");
    let (out, err, code) = run("while :; do\n echo a\n break\n done && echo x");
    assert_eq!((out.as_str(), code), ("a\nx\n", 0), "stderr={err}");
    let (out, err, code) = run("if true; then\necho b\nfi && echo y");
    assert_eq!((out.as_str(), code), ("b\ny\n", 0), "stderr={err}");
    let (out, err, code) = run("if false; then\necho b\nfi && echo y");
    assert_eq!((out.as_str(), code), ("y\n", 0), "stderr={err}");
}

/// The nested multi-line rows of the task: a multi-line block whose body
/// carries a nested block with a trailing operator. The heredoc-free text
/// keeps the block opaque (the nested `fi` scores inside, LESSONS #137 gate
/// 3), so the `&&` is the body's own cond list; the failing-gate row prints
/// nothing — its rc 1 (bash rc 0) is task #262's pinned `break`-status
/// divergence, not this shape's.
#[test]
fn nested_multi_line_blocks_keep_their_operators_inside() {
    let (out, err, code) = run("while :; do\nif true; then echo t; fi && echo x\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("t\nx\n", 0), "stderr={err}");
    let (out, err, code) = run("while :; do\nif true; then false; fi && echo x\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}");
}

/// `set -e` rows of the task: the errexit rules around mid-run blocks are
/// the list's own — a skipped chain never fires it, the run's block status
/// does not either. Measured: bash prints `after` rc 0 on every row.
#[test]
fn errexit_does_not_fire_around_midrun_blocks() {
    for script in [
        "set -e; true && while :; do echo hi; break; done; echo after",
        "set -e; false && while :; do echo hi; break; done; echo after",
        "set -e; false && if true; then echo t; fi; echo after",
        "set -e; true && case x in x) echo cx;; esac; echo after",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "{script}: stderr={err}");
        assert!(out.ends_with("after\n"), "{script}: {out}");
    }
}

/// The mid-run heredoc pin of LESSONS ("A cond-list heredoc arm's
/// `b == b'\\n'` guard is a mutation survivor with no mid-run pin"): a `<<`
/// written mid-run whose body follows. The body's `&&` must not split the
/// part; the split is the operator written before the body. Measured: bash
/// `q && r` `b` rc 0, fdshell identical. This row kills the pre-existing
/// `==`→`!=` survivor in `cond/boundary.rs` (its follow-up bookkeeping is
/// task #248's).
#[test]
fn mid_run_heredoc_keeps_the_body_inside_the_part() {
    let (out, err, code) = run("cat <<X && echo b\nq && r\nX");
    assert_eq!((out.as_str(), code), ("q && r\nb\n", 0), "stderr={err}");
}

/// A mid-run block that carries a here-doc in its body claims the body
/// through the run: `cat` prints it and the loop continues (measured against
/// bash; the region bookkeeping of #138/#238 applies to the mid-run run
/// body unchanged).
#[test]
fn midrun_block_body_heredoc_travels_with_the_run() {
    let (out, err, code) = run("true && while :; do cat <<T; echo mid; break; done\nbody\nT");
    assert_eq!((out.as_str(), code), ("body\nmid\n", 0), "stderr={err}");
}

/// Review F1 (task #232): a POSIX builtin `wait` written mid-run opens no
/// block, so it must stay **word data** — the mid-run span is gated on the
/// shared scorer (`keyword_delta` scores the builtin `0`, only the event-case
/// `wait` `+1`). Without that gate the builtin took a span, found no closer,
/// and swallowed the rest of the run; the cond layer re-splits only `&&`/`||`
/// and never a `;`, so every statement after the first `;` of the run was
/// dropped with rc 0 and no diagnostic, and a loop whose `break` sat in the
/// swallowed text ran forever (that row is pinned bounded in
/// `tests/loops.rs::mid_run_builtin_wait_keeps_the_loop_body`). Rows measured
/// against bash 5.3.9; `master` agreed with bash on all of them.
#[test]
fn builtin_wait_mid_run_stays_word_data() {
    for (script, want) in [
        ("true && wait; echo tail", "tail\n"),
        ("echo a && wait; echo b", "a\nb\n"),
        ("true && echo x && wait; echo tail", "x\ntail\n"),
        ("true && time wait; echo tail", "tail\n"),
        ("true && time -p wait; echo tail", "tail\n"),
        ("false && wait; echo tail", "tail\n"),
        ("true || wait; echo tail", "tail\n"),
        ("set -e; true && wait; echo tail", "tail\n"),
        ("if true; then true && wait; echo tail; fi", "tail\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The gated opener keeps its span mid-run: the **event-case** `wait` (a
/// pattern keyword behind it, fdshell-only syntax so bash is a syntax error
/// here) scores `+1`, so `true &&` runs the block and `false &&` skips it —
/// the span and the short circuit, pinned beside the builtin row above that
/// must *not* take one. Measured on `master` the same script ran `wait` as the
/// builtin and `finished` as a command (`failed to resolve command path`).
#[test]
fn event_case_wait_mid_run_keeps_its_block_span() {
    let (out, err, code) =
        run("builtin true &>&j; true && wait\n finished %&j)\n echo f ;;\ndone; echo tail");
    assert_eq!((out.as_str(), code), ("f\ntail\n", 0), "stderr={err}");
    let (out, err, code) =
        run("builtin true &>&j; false && wait\n finished %&j)\n echo f ;;\ndone; echo tail");
    assert_eq!((out.as_str(), code), ("tail\n", 0), "stderr={err}");
}

/// Accepted divergences pinned with the task that owns them: a keyword block
/// as a pipeline stage (argument position — Fix A must not cover it, task
/// **#292**); an unclosed mid-run construct (rc 1 parse error, bash rc 2); a
/// line that opens with `&&` (bash rejects it, rc 2; fdshell runs the part,
/// documented in README, no task); the heredoc-carrying block text (task
/// **#234**).
#[test]
fn midrun_adjacent_shapes_stay_measured_divergences() {
    // #292: the `|` position is argument position — `while`/`do` stay words.
    let (_out, _err, code) = run("echo a | while :; do echo b; break; done");
    assert_eq!(code, 1);
    // Unclosed mid-run: parse error rc 1 (bash rc 2).
    let (_out, _err, code) = run("true && while :; do echo hi");
    assert_eq!(code, 1);
    let (_out, _err, code) = run("true && while");
    assert_eq!(code, 1);
    let (_out, _err, code) = run("true && if");
    assert_eq!(code, 1);
    // A line opening with `&&`: bash rc 2 (syntax), fdshell runs the part.
    let (out, _err, code) = run("while :; do\necho a\nbreak\ndone\n&& echo x");
    assert_eq!(
        (out.as_str(), code),
        ("a\nx\n", 0),
        "bash rc 2: measured divergence"
    );
    // A `name()` word with no `{` body opens no definition span (the scorer
    // gate, LESSONS): it stays word data, rc 1 — byte-identical to master and
    // to the run-start twin `f()`. bash reports a syntax error rc 2.
    let (_out, _err, code) = run("true && f()");
    assert_eq!(code, 1);
    // #234: a block text carrying a `<<` operator keeps the old split.
    let (_out, _err, code) = run("if true; then cat <<T; fi && echo x\nbody\nT");
    assert_eq!(code, 1);
}

/// Task #137 regression: a keyword-shaped word in an **argument** position is
/// word data, never a block opener — the same command-position rule the group
/// arm and the segment scanner apply. Without that gate the word scored `+1`,
/// the block depth left 0, and every later `&&` / `||` of the part was swallowed
/// into the command's arguments: `echo if && echo b` printed one argument
/// `if && echo b`, `echo if || echo b` failed with `unexpected pipe` rc 1,
/// `test if && echo b` ran a 4-argument `test` (rc 2), and
/// `echo x > if && echo b` wrote `x && echo b` into the file `if` and never ran
/// `echo b` — data corruption. Every row is measured against `bash -c` 5.3.9.
#[test]
fn keyword_shaped_argument_words_do_not_swallow_the_cond_operators() {
    for (script, want) in [
        ("echo if && echo b", "if\nb\n"),
        ("echo while && echo b", "while\nb\n"),
        ("echo for && echo b", "for\nb\n"),
        ("echo case && echo b", "case\nb\n"),
        ("echo until && echo b", "until\nb\n"),
        // A keyword-shaped *closer* is an argument too.
        ("echo fi && echo b", "fi\nb\n"),
        ("echo done && echo b", "done\nb\n"),
        // `||` short-circuits on the successful first part.
        ("echo if || echo b", "if\n"),
        ("echo if && echo b && echo c", "if\nb\nc\n"),
        ("true && echo if && echo b", "if\nb\n"),
        ("echo x | echo if && echo b", "if\nb\n"),
        // A one-argument `test` on a non-empty word is true, as in bash.
        ("test if && echo b", "b\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
    // A redirect target is an argument: the file holds the command's output
    // alone, and the second part runs.
    let dir = scratch("keyword-redirect");
    let (out, err, code) = run_in(&dir, "echo x > if && echo b");
    assert_eq!((out.as_str(), code), ("b\n", 0), "stderr={err}");
    assert_eq!(std::fs::read_to_string(dir.join("if")).unwrap(), "x\n");
    std::fs::remove_dir_all(&dir).unwrap();
}

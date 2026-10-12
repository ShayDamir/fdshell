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

/// Accepted divergence, task #205: the shape is now *parsed*, and a `break`
/// written in a loop condition is dropped (the condition runs as a cond list and
/// its loop control does not abort the loop). bash exits the loop (`i=0`);
/// fdshell runs every iteration (`i=3`).
#[test]
fn break_in_an_and_condition_is_ignored() {
    let (out, _err, code) = run("i=0; while [ $i -lt 3 ] && break; do i=$((i+1)); done; echo i=$i");
    assert_eq!(code, 0);
    assert_eq!(out, "i=3\n");
}

/// Known gap, task #232: an operator *adjacent* to a block (outside its text)
/// is unchanged by task #137 — `true && while …` is still a parse error rc 1
/// (bash `hi` rc 0).
#[test]
fn operator_adjacent_to_a_block_is_unchanged() {
    let (_out, _err, code) = run("true && while [ 1 ]; do echo hi; break; done");
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

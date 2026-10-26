#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;
use sys::Pid;
use sys::signal::{SIGKILL, kill};

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

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it
/// runs on a failing test's unwind, which is exactly the test that used to
/// leak.
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

fn scratch(tag: &str) -> Scratch {
    let dir =
        std::env::temp_dir().join(format!("fdshell-cond_blocks-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// A file the child wrote in its scratch dir (a background job's pid, its body,
/// the parent's `pwd`): the scratch-dir idiom of
/// [`ampersand_in_an_if_body_backgrounds_the_statement`] — stdout is shared with
/// every background child, so a file is the only race-free channel. `""` when
/// the child never wrote it (a preflight parse error runs nothing).
fn record(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_default()
}

// Task #236 section: keyword words in argument position inside a block body.
// Every shape is measured against `bash -c` (5.3.9). The `echo done` row of
// [`keyword_words_in_argument_position_split_the_body_operators`] runs under
// a bounded child: on master the `break` landed in a dropped cond part and
// the loop ran forever (rc 124), so a regression must panic, not hang — the
// same harness as `tests/loops.rs::run_bounded`.

/// 5 s, strictly under nextest's 10 s `slow-timeout` (`.config/nextest.toml`).
const BOUND: std::time::Duration = std::time::Duration::from_secs(5);

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
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + BOUND;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => panic!("try_wait failed: {e}"),
        }
        if std::time::Instant::now() >= deadline {
            for pid in children_of(child.id()) {
                let _ = kill(Pid::from_raw(pid as i32), SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "`{script}` did not finish in {BOUND:?} — the loop-control `break` was dropped again (task #205's shape regressed through #236's gate)"
            )
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    (
        str::from_utf8(&out.stdout).unwrap().to_string(),
        str::from_utf8(&out.stderr).unwrap().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

/// Run every `script` at once, capped at [`BOUND`] each, and return their exit
/// codes in order — the pinned-hang harness: rows whose measured fdshell
/// result *is* a body that never returns (README "Accepted divergences", task
/// #236/#309). Unlike [`run_bounded`] a timeout is the *expected* outcome, so
/// it returns rc 124 instead of panicking; a row that stops hanging is
/// reported as its real rc. Running the family in parallel keeps the
/// twelve-row pin well inside nextest's 10 s `slow-timeout`.
fn run_capped(scripts: &[&str]) -> Vec<i32> {
    let mut children: Vec<(std::process::Child, String)> = scripts
        .iter()
        .map(|script| {
            let child = Command::new(BIN)
                .args(["-c", script])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            (child, (*script).to_string())
        })
        .collect();
    let deadline = std::time::Instant::now() + BOUND;
    let mut codes = Vec::new();
    for (child, script) in children.iter_mut() {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    codes.push(status.code().unwrap_or(-1));
                    break;
                }
                Ok(None) => {}
                Err(e) => panic!("try_wait failed on `{script}`: {e}"),
            }
            if std::time::Instant::now() >= deadline {
                for pid in children_of(child.id()) {
                    let _ = kill(Pid::from_raw(pid as i32), SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                codes.push(124);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    codes
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

/// Task #234: a heredoc-carrying block text is opaque to the cond split too —
/// the `&&` in the block's **command** text is the block's own cond list. What
/// stays a divergence is the shape whose heredoc has **no delimiter line**
/// inside the block text: the block runs as one part and reports its own
/// unterminated heredoc (rc 1), where bash reports a syntax error rc 2 (the
/// same rc-1-vs-2 family as `midrun_adjacent_shapes_stay_measured_divergences`).
#[test]
fn and_in_a_block_with_an_unterminated_heredoc_is_a_parse_error() {
    for script in [
        "while cat <<X; do a && b; X\ndone\necho after",
        "if cat <<X; then a && b; X\nfi\necho after",
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: bash rc 2");
        assert!(
            err.contains("missing terminating delimiter line 'X'"),
            "{script}: stderr={err}"
        );
    }
}

/// Task #234: the shapes the gate used to break. A heredoc in a block
/// condition (single-line, so its bodies sit past the closing keyword) with an
/// `&&` in the block's command text now runs the block's own cond list. Rows
/// measured against bash 5.3.9. The `for`/`case` twins of this shape were the
/// closing-keyword/region defect task **#273** owns: its `for`
/// and `case` twins (`for q in 1; do cat <<Q; echo a && echo b; done` and
/// `case q in q) cat <<Q; echo a && echo b;; esac` + body + `Q`) were rc 1
/// `expected 'done'` / `case: missing 'esac'` on `master`, where bash prints
/// `body` `a` `b` rc 0. #234 widens nothing there; #273 lands the parity, and
/// the rows below are the shape measured on both operators (bash 5.3.9).
#[test]
fn and_in_a_block_that_carries_a_heredoc_runs_the_block() {
    for (script, want) in [
        (
            "while cat <<X; do echo a && echo b; break; done\nbody\nX",
            "body\na\nb\n",
        ),
        (
            "if cat <<X; then echo a && echo b; fi\nbody\nX",
            "body\na\nb\n",
        ),
        // The operator *outside* the block, on a block whose bodies sit past
        // `fi`: the part that owns the `<<` gets its own body (task #234).
        ("if true; then cat <<T; fi && echo x\nbody\nT", "body\nx\n"),
        // #273's parity: the `for` and `case` twins of the same shape.
        (
            "for q in 1; do cat <<Q; echo a && echo b; done\nbodyQ\nQ",
            "bodyQ\na\nb\n",
        ),
        (
            "case q in q) cat <<Q; echo a && echo b;; esac\nbodyQ\nQ",
            "bodyQ\na\nb\n",
        ),
        (
            "for q in 1; do cat <<Q; echo a || echo b; done\nbodyQ\nQ",
            "bodyQ\na\n",
        ),
        (
            "case q in q) cat <<Q; echo a || echo b;; esac\nbodyQ\nQ",
            "bodyQ\na\n",
        ),
        (
            "true && for q in 1; do cat <<Q; echo a || echo b; done\nbodyQ\nQ",
            "bodyQ\na\n",
        ),
        (
            "true && case q in q) cat <<Q; echo a || echo b;; esac\nbodyQ\nQ",
            "bodyQ\na\n",
        ),
        (
            "for x in a; do cat <<T; done || echo after\nbodyT\nT",
            "bodyT\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
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
/// nothing and its rc is the `break`'s 0 (bash's, measured 2026-10-19 — the
/// status gap this row used to pin as 1 was task #262's, now fixed).
#[test]
fn nested_multi_line_blocks_keep_their_operators_inside() {
    let (out, err, code) = run("while :; do\nif true; then echo t; fi && echo x\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("t\nx\n", 0), "stderr={err}");
    let (out, err, code) = run("while :; do\nif true; then false; fi && echo x\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
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

/// Accepted divergences pinned with the task that owns them: an unclosed
/// mid-run construct (rc 1 parse error, bash rc 2); a line that opens with
/// `&&` (bash rejects it, rc 2; fdshell runs the part, documented in README,
/// no task). The keyword block **as a pipeline stage** used to sit in this
/// list (task #292): the `|` position is a command position, so that script
/// now forks the block as a stage and matches bash — pinned with the family in
/// `tests/pipeline.rs` (`block_stage_*`).
#[test]
fn midrun_adjacent_shapes_stay_measured_divergences() {
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
}

/// Task #295: a **multi-line** mid-run construct whose body carries a here-doc
/// stays a measured divergence. The construct's body region starts on its own
/// lines, so it swallows the closer (`break` / `done`, `}`) and the construct
/// reports its own unterminated shape — rc 1, where bash reports a syntax
/// error rc 2. Same rc-1-vs-2 family as
/// `midrun_adjacent_shapes_stay_measured_divergences` above and as
/// `tests/heredoc.rs::block_body_heredoc_delimiter_line_operator_is_region_data`
/// (task #234); the **single-line** mid-run twin is bash-equal and pinned in
/// `tests/heredoc.rs::mid_run_heredoc_parity_family`. No task owns this family
/// yet — the pins document it and README covers it in words.
#[test]
fn multi_line_mid_run_constructs_with_a_body_heredoc_stay_measured() {
    let (_out, err, code) = run("true && while :; do\ncat <<T; break\ndone\nbodyT\nT");
    assert_eq!(code, 1, "bash rc 2: syntax error from `while'");
    assert!(err.contains("expected 'done'"), "stderr={err}");
    let (_out, err, code) = run("true && f() {\ncat <<T\n}\nbodyT\nT\nf");
    assert_eq!(code, 1, "bash rc 2: syntax error from `{{'");
    assert!(
        err.contains("function definition is missing a closing brace"),
        "stderr={err}"
    );
    let (_out, err, code) = run("true && {\ncat <<T\n}\nbodyT\nT");
    assert_eq!(code, 1, "bash rc 2: syntax error from `{{'");
    assert!(
        err.contains("group is missing a closing brace"),
        "stderr={err}"
    );
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
}

/// Task #235: `&&` / `||` written in a function definition's **body** are body
/// data — the definition is one cond-list part (the group counter counts the
/// definer's body brace, `cond/boundary::opens_group`), the call runs the
/// body's own cond list. Every definer spelling, the glued-brace-with-space
/// form, the multi-line form, a nested group / keyword block in the body, a
/// group around the definition, a definition in an `if` body, and a variable
/// word in the body are measured against `bash -c` 5.3.9.
///
/// Not pinned here: `f() { … }; f | cat` — the `&&` split is fixed, but a
/// function *in a pipeline stage* is task #182's pre-existing gap (`f | cat`
/// fails identically with a `&&`-free body on master).
#[test]
fn definer_body_operators_keep_the_definition_one_part() {
    for (script, want) in [
        ("f() { echo a && echo b; }; f", "a\nb\n"),
        ("f(){ echo a && echo b; }; f", "a\nb\n"),
        ("function f { echo a && echo b; }; f", "a\nb\n"),
        ("function f(){ echo a && echo b; }; f", "a\nb\n"),
        ("function f() { echo a && echo b; }; f", "a\nb\n"),
        ("function f () { echo a && echo b; }; f", "a\nb\n"),
        ("function f ( ) { echo a && echo b; }; f", "a\nb\n"),
        ("f() {\necho a && echo b\n}; f", "a\nb\n"),
        ("g=1; f() { echo $g && echo b; }; f", "1\nb\n"),
        ("f() { { echo a && echo b; }; }; f", "a\nb\n"),
        ("f() { if true; then echo t && echo u; fi; }; f", "t\nu\n"),
        ("{ f() { echo a && echo b; }; }; f", "a\nb\n"),
        ("if true; then f() { echo a && echo b; }; fi; f", "a\nb\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The cond-list operators written *around* a definition stay top-level: the
/// definition is one part and its status gates the operator, exactly as the
/// keyword blocks' `trailing_operator_after_a_midrun_block_splits_the_list`
/// case. The short-circuited definer never lands (bash 127 vs fdshell's
/// `f: not found` rc 1 — the accepted command-not-found divergence).
#[test]
fn definer_with_cond_operators_around_it_splits_the_list() {
    let (out, err, code) = run("f() { echo a && echo b; } && echo c; f");
    assert_eq!((out.as_str(), code), ("c\na\nb\n", 0), "stderr={err}");
    let (out, err, code) = run("f() { echo a && echo b; } ; f && echo z");
    assert_eq!((out.as_str(), code), ("a\nb\nz\n", 0), "stderr={err}");
    let (out, err, code) = run("true && f() { echo a && echo b; } && f");
    assert_eq!((out.as_str(), code), ("a\nb\n", 0), "stderr={err}");
    let (out, err, code) = run("false || f() { echo a && echo b; }; f");
    assert_eq!((out.as_str(), code), ("a\nb\n", 0), "stderr={err}");
    let (_out, err, code) = run("false && f() { echo a && echo b; }; f");
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("\"f\" not found"), "stderr={err}");
    let (out, err, code) = run("f() { echo a; { echo b && echo c; } && echo d; }; f");
    assert_eq!((out.as_str(), code), ("a\nb\nc\nd\n", 0), "stderr={err}");
    let (out, err, code) = run("f() { false && echo b; }; f; echo rc=$?");
    assert_eq!((out.as_str(), code), ("rc=1\n", 0), "stderr={err}");
    let (out, err, code) = run("f() { echo a && echo b; }; f; f");
    assert_eq!((out.as_str(), code), ("a\nb\na\nb\n", 0), "stderr={err}");
}

/// Task #236: a keyword-shaped word in **argument position inside a block
/// body** is word data (bash recognizes a reserved word in command position
/// only), so the body's `&&` / `||` keeps splitting. On `master` the argument
/// opened (or dropped) the block depth and the whole shape was a parse error
/// rc 1 — the `while … echo done …` row even **hung** (the `break` landed in a
/// dropped cond part, the #205 lesson), so it runs under the bounded child.
/// Rows measured against `bash -c` 5.3.9: every row prints its keyword word,
/// the gated part, and `after`, rc 0.
#[test]
fn keyword_words_in_argument_position_split_the_body_operators() {
    for (script, want) in [
        (
            "while true; do echo if && echo x; break; done; echo after",
            "if\nx\nafter\n",
        ),
        (
            "if true; then echo while && echo x; fi; echo after",
            "while\nx\nafter\n",
        ),
        (
            "case x in x) echo if && echo y;; esac; echo after",
            "if\ny\nafter\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
    // The closer twin: `echo done` is data, the real `done` closes, and the
    // body's `&&` runs. Master hung (rc 124) — bounded child, so a regression
    // panics with the script, never stalls the suite.
    let (out, err, code) =
        run_bounded("while true; do echo done && echo x; break; done; echo after");
    assert_eq!(
        (out.as_str(), code),
        ("done\nx\nafter\n", 0),
        "stderr={err}"
    );
    let (out, err, code) = run("if true; then echo done && echo x; fi; echo after");
    assert_eq!(
        (out.as_str(), code),
        ("done\nx\nafter\n", 0),
        "stderr={err}"
    );
    // No operator at all: the argument-keyword is a plain argument (master
    // reported the unmatched-keyword parse error rc 1).
    let (out, err, code) = run("while true; do echo if; break; done; echo after");
    assert_eq!((out.as_str(), code), ("if\nafter\n", 0), "stderr={err}");
}

/// The clause positions of task #236: `then`/`else`/`elif` bodies, a `case`
/// arm body, and an `until` body all put their words in the same clause rule —
/// a keyword-shaped argument is data there too, and a nested block in a body
/// keeps its operators inside the block. Rows measured against bash.
#[test]
fn clause_positions_apply_the_same_rule() {
    for (script, want) in [
        (
            "if :; then echo a; elif :; then echo if; fi; echo after",
            "a\nafter\n",
        ),
        ("if :; then :; else echo while; fi; echo after", "after\n"),
        ("case x in a) echo do;; esac", ""),
        ("until :; do echo if; break; done; echo after", "after\n"),
        (
            "while true; do if true; then false && echo b; fi; break; done; echo after",
            "after\n",
        ),
        (
            "while :; do case x in a) if :; then :; fi;; esac; break; done; echo after",
            "after\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// A multi-line body keeps the rule across the run resets, and an operator
/// written **after** the block's closer is still the outer cond list's
/// (#232's mid-run rule survives the new flag). Rows measured against bash.
#[test]
fn multi_line_bodies_and_trailing_operators_keep_the_rule() {
    let (out, err, code) = run("while :; do\necho if && echo x\nbreak\ndone\necho after");
    assert_eq!((out.as_str(), code), ("if\nx\nafter\n", 0), "stderr={err}");
    let (out, err, code) = run("while :; do echo if; break; done && echo tail");
    assert_eq!((out.as_str(), code), ("if\ntail\n", 0), "stderr={err}");
    let (out, err, code) = run("true && while :; do echo if; break; done && echo tail");
    assert_eq!((out.as_str(), code), ("if\ntail\n", 0), "stderr={err}");
}

/// A closer written in argument position is word data: `echo fi` prints `fi`
/// and the block closes at its real `fi` (master: rc 1 — the argument-`fi`
/// closed the block early and cut the body). Rows measured against bash.
#[test]
fn closers_in_argument_position_stay_data() {
    for (script, want) in [
        ("if :; then echo fi; fi; echo after", "fi\nafter\n"),
        ("if :; then echo esac; fi; echo after", "esac\nafter\n"),
        (
            "f() { if :; then echo if; fi; }; f; echo after",
            "if\nafter\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The `case` list position (task #236's `Place::Pattern`): `esac` is the
/// list's terminator and scores (`case x in esac` stays a complete `case`),
/// while an opener-shaped **pattern word** (`if)`) is a pattern, not a block —
/// the pin that kills "closers and openers share one gate". Measured: bash
/// prints only `after` for the pattern row, rc 0 on both.
#[test]
fn case_list_position_scores_closers_not_openers() {
    let (out, err, code) = run("case x in esac; echo after");
    assert_eq!((out.as_str(), code), ("after\n", 0), "stderr={err}");
    let (out, err, code) = run("case x in if) echo p;; esac; echo after");
    assert_eq!((out.as_str(), code), ("after\n", 0), "stderr={err}");
}

/// Accepted divergences pinned as measured (task #236):
/// * the glued-closer forms (`while … echo x done`, `if a; then b fi`,
///   `if test fi`, `case … *) echo no esac`) become **unclosed constructs**:
///   the closer behind a command word never scores, so the block swallows the
///   line and the run reports the parse error rc 1 where bash reports a syntax
///   error rc 2 — README "unclosed mid-run construct" (#294's family). The
///   parse layer's detailed messages for these shapes are pinned in
///   `parse::tests` and `tests/block_bodies.rs`.
/// * `while :; do : done` is the same family's **hang** row: on master the
///   loop never closes and runs forever (rc 124); the closer-in-argument rule
///   leaves it unclosed, so it is a parse error rc 1 (bash rc 2). Bounded
///   child: a regression back into the loop panics with the script.
/// * `echo { if && echo b` is unchanged top level: the group arm's own
///   clause rule keeps `echo {` word data (the brace carve-out is the in-block
///   half; `tests/group.rs::literal_brace_argument_in_a_block_body` pins the
///   body-position asymmetry `echo }` eats the closer — #189/#193's rule).
#[test]
fn glued_closer_forms_stay_measured_divergences() {
    for script in [
        "while true; do echo x done",
        "if a; then b fi",
        "if test fi",
        "case \"x\" in a) echo yes;; *) echo no esac",
        "for i in a; do echo done",
    ] {
        let (out, _err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", 1), "{script}: bash rc 2");
    }
    let (out, err, code) = run_bounded("while :; do : done");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}: bash rc 2");
    let (out, err, code) = run("echo { if && echo b");
    assert_eq!((out.as_str(), code), ("{ if\nb\n", 0), "stderr={err}");
}

/// Task #236 closing #237's gap: a `#`-comment line in a block body is inert
/// for the block counter — the words behind `#` sit in argument position, so
/// `# done` never drops the depth and the body's `&&` stays the body's own
/// (measured against bash: `a`, `b`, rc 0; master reported `expected 'done'`,
/// rc 1). The heredoc-carrying twin is pinned in
/// `tests/heredoc.rs::comment_block_closer_in_a_block_body_line_is_inert`.
#[test]
fn comment_lines_in_a_block_body_are_inert() {
    let (out, err, code) = run("while :; do # done\necho a && echo b\nbreak\ndone; echo after");
    assert_eq!((out.as_str(), code), ("a\nb\nafter\n", 0), "stderr={err}");
}

/// Task #236 (round-3 F1): a word that only *looks* like an in-block opener
/// must not move the command-position flag. `Place::Pattern` is the `case`/`for`
/// word list, and bash opens it on the single word `in` — a generalization to
/// every `i`-shaped payload (`is`, `it`, `ix`) turns an argument into a pattern
/// place, and the closer written after it (`done`/`fi`/`esac`) scores there, so
/// the enclosing block closes early: the body loses its `break` (the loop runs
/// forever) or its tail. Measured against `bash -c` (5.3.9): every row prints
/// its words and exits 0. Bounded children: the regression measured at
/// `cb9febb` on the first four rows was a hang, so a regression must fail with
/// the script, not stall the suite. `while :; do echo in done; …` is *not* in
/// this test: with `in` → `Place::Pattern` it hung on master, and task #301's
/// `;`-before-closer rule turned that hang into the parse error pinned by
/// [`in_word_list_argument_before_a_closer_is_a_parse_error`].
#[test]
fn keyword_shaped_arguments_that_only_look_like_an_in_stay_data() {
    for (script, want) in [
        (
            "while :; do echo is done; break; done; echo after",
            "is done\nafter\n",
        ),
        (
            "while :; do echo is fi; break; done; echo after",
            "is fi\nafter\n",
        ),
        (
            "while :; do echo ix done; break; done; echo after",
            "ix done\nafter\n",
        ),
        (
            "while :; do echo it esac; break; done; echo after",
            "it esac\nafter\n",
        ),
        (
            "while :; do echo i done; break; done; echo after",
            "i done\nafter\n",
        ),
        (
            "if :; then echo is done; fi; echo after",
            "is done\nafter\n",
        ),
        ("case x in a) echo is done;; esac; echo after", "after\n"),
    ] {
        let (out, err, code) = run_bounded(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// Task #309 (README "body that never returns"): the pairing of an open block
/// with its closer is **typed** (`fi`↔`if`, `done`↔`while`/`until`/`for`/`wait`,
/// `esac`↔`case`, `}`↔`{`), so a closer of a foreign kind written in a body is
/// an ordinary **word**: it scores `0`, it leaves the command-position flag at
/// `Data`, and it does not end the enclosing block. The body therefore keeps
/// the tail that the type-blind depth used to swallow — the loop's `break`
/// lands and the script ends.
///
/// The **rule** is the family, not this list, and this pin is a *sample* of it:
/// the argument-position opener of task #236 plus a closer of the wrong kind is
/// the shape, and its outcome depends only on which closers are left after it
/// (word data → this test; no closer of the block's own kind →
/// [`a_mismatched_closer_that_leaves_no_closer_is_a_parse_error`]; a matching
/// closer that ends the loop above its `break` →
/// [`a_loop_body_that_never_returns_where_bash_refuses_the_line`]).
///
/// Bounded children throughout: a regression back into the hang must fail with
/// the script, not stall the suite. bash 5.3.9 refuses most of these lines
/// (rc 2) — `if`/`fi`/`done`/`esac` are reserved words in its grammar, so the
/// line never runs. fdshell runs the closer as a command
/// (`failed to resolve command path: "fi"`, rc 127 inside the body) and the
/// script often exits 0: the accepted rc-0 acceptance divergence of the
/// stray-closer family (`tests/block_closers.rs::the_rc_1_family_stays_a_parse_error`).
#[test]
fn mismatched_closers_in_a_loop_body_are_word_data() {
    for (script, want) in [
        (
            "while :; do echo if; fi; echo x; break; done; echo after",
            "if\nx\nafter\n",
        ),
        (
            "while :; do echo case; esac; break; done; echo after",
            "case\nafter\n",
        ),
        (
            "while :; do echo until; fi; break; done; echo after",
            "until\nafter\n",
        ),
        (
            "while :; do echo while; fi; break; done; echo after",
            "while\nafter\n",
        ),
        (
            "while :; do echo if; echo x; fi; break; done; echo after",
            "if\nx\nafter\n",
        ),
        (
            "while :; do echo a if; fi; echo x; break; done; echo after",
            "a if\nx\nafter\n",
        ),
        (
            "while :; do echo if; esac; break; done; echo after",
            "if\nafter\n",
        ),
        (
            "while :; do echo if; fi; echo x; echo y; break; done; echo after",
            "if\nx\ny\nafter\n",
        ),
        (
            "while :; do echo done; fi; break; done; echo after",
            "done\nafter\n",
        ),
        (
            "while :; do echo fi; fi; break; done; echo after",
            "fi\nafter\n",
        ),
        (
            "while :; do echo esac; esac; break; done; echo after",
            "esac\nafter\n",
        ),
        (
            "while :; do echo if && echo z; fi; break; done; echo after",
            "if\nz\nafter\n",
        ),
        (
            "while :; do echo if || echo z; fi; break; done; echo after",
            "if\nafter\n",
        ),
        (
            "while :; do echo if; fi && echo z; break; done; echo after",
            "if\nafter\n",
        ),
        ("if :; then echo if; fi; done; echo after", "if\nafter\n"),
        (
            "while :; do echo \"if\"; fi; break; done; echo after",
            "if\nafter\n",
        ),
        (
            "while :; do echo if; time fi; break; done; echo after",
            "if\nafter\n",
        ),
        ("f() { echo if; fi; }; f; echo after", "if\nafter\n"),
        (
            "while :; do while :; do echo if; fi; break; done; break; done; echo after",
            "if\nafter\n",
        ),
        (
            "while :; do for x in a; do echo if; fi; break; done; break; done; echo after",
            "if\nafter\n",
        ),
        (
            "while :; do echo a | echo if; fi; break; done; echo after",
            "if\nafter\n",
        ),
        (
            "while :; do echo if; fi | echo x; break; done; echo after",
            "if\nx\nafter\n",
        ),
        (
            "for x in a; do echo if; fi; break; done; echo after",
            "if\nafter\n",
        ),
        (
            "for ((i=0;i<3;i++)); do echo if; fi; done; echo after",
            "if\nif\nif\nafter\n",
        ),
        ("case x in a) echo if; fi;; esac; echo after", "after\n"),
        ("case x in a) echo a; done;; esac; echo after", "after\n"),
        (
            "while :; do case x in a) echo if; fi;; esac; break; done; echo after",
            "after\n",
        ),
        (
            "if :; then echo fi; echo done; fi; echo after",
            "fi\ndone\nafter\n",
        ),
    ] {
        let (out, err, code) = run_bounded(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
    // The stray closer is a *command*: the body runs, `fi` resolves to nothing,
    // and the script still ends (bash rc 2).
    let (out, err, code) =
        run_bounded("while :; do echo a; fi done; echo x; break; done; echo after");
    assert_eq!((out.as_str(), code), ("a\nx\nafter\n", 0), "stderr={err}");
    assert!(
        err.contains("failed to resolve command path"),
        "stderr={err}"
    );
    // A brace argument in a body is the same rule at the brace kind: a `}` is
    // the closer of a **brace** block, so in a `while`/`if` body it is a word
    // (bash's rc 0 too — the row `tests/group.rs` pins for the group side).
    let (out, err, code) = run_bounded("while :; do echo }; break; done; echo after");
    assert_eq!((out.as_str(), code), ("}\nafter\n", 0), "stderr={err}");
}

/// Task #309, the half that becomes a report: a closer of a foreign kind is a
/// word, so a block whose **own** closer is gone never closes and the
/// "unclosed mid-run construct" report names it (README, task #294's family).
/// `master` closed the block at the foreign closer and ran the line (several of
/// these rows were the hang of [`mismatched_closers_in_a_loop_body_are_word_data`]'s
/// family before the pairing). bash refuses every row (rc 2); fdshell's rc 1 is
/// the rc-1-vs-2 family with an actionable message. Sample of the rule, not its
/// extent; bounded children, because the pre-fix shape of several rows is a
/// hang.
#[test]
fn a_mismatched_closer_that_leaves_no_closer_is_a_parse_error() {
    for (script, want) in [
        (
            "while :; do echo a; fi; echo after",
            "'while' is missing its closing 'done'",
        ),
        (
            "while :; do echo if; fi; echo after",
            "'while' is missing its closing 'done'",
        ),
        (
            "while :; do echo if; fi done; echo after",
            "'while' is missing its closing 'done'",
        ),
        (
            "while :; do echo a; esac; echo after",
            "'while' is missing its closing 'done'",
        ),
        (
            "while :; do if :; then :; done fi; echo after",
            "'while' is missing its closing 'done'",
        ),
        (
            "while :; do if :; then echo a; done; fi; echo after",
            "'while' is missing its closing 'done'",
        ),
        (
            "while :; do if; done; break; done; echo after",
            "'while' is missing its closing 'done'",
        ),
        (
            "if :; then echo a; done; echo after",
            "'if' is missing its closing 'fi'",
        ),
        ("{ echo a; fi; echo after", "'{' is missing its closing '}'"),
    ] {
        let (_out, err, code) = run_bounded(script);
        assert_eq!(code, 1, "{script}: stderr={err}, bash rc 2");
        assert!(err.contains(want), "{script}: stderr={err}");
    }
    // The `}` that ends a group opens the group scanner's own rule: the group
    // body keeps its clause and the group's builder answers, not the `while`.
    let (_out, err, code) = run_bounded("while :; do { echo if; } fi; break; done; echo after");
    assert_eq!(code, 1, "stderr={err}, bash rc 2");
    assert!(err.contains("group: unexpected word after '}'"), "{err}");
}

/// Task #309 lands part of task **#310**'s word-list rule for free: the scans
/// are seeded with the closer of the block they enter, so a `done` of a `case`
/// **pattern** list mismatches the `case`'s `esac` and is a list word — the
/// block reaches its real `esac` and the row is **bash parity** (rc 0). The
/// byte-level and token-level scans agree (the seed `Closer::Esac` reaches
/// `parse::case_block` too); the rows
/// `tests/heredoc.rs::closer_shaped_case_word_list_words_reach_the_real_closer`
/// and `tests/loops.rs::closer_shaped_list_words_keep_the_measured_messages_and_the_seed_is_for_only`
/// pin the rest. #310 owns what is left of the list rule at the byte level.
#[test]
fn a_closer_shaped_case_pattern_list_word_is_a_word() {
    for (script, want) in [
        (
            "while :; do case x in done) echo p;; esac; break; done; echo after",
            "after\n",
        ),
        (
            "if :; then case x in done) echo p;; esac; fi; echo after",
            "after\n",
        ),
        ("case done in done) echo p;; esac", "p\n"),
    ] {
        let (out, err, code) = run_bounded(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// Task #309, the **bash-parity** half of the "body that never returns" family
/// (README): a body that ends at a closer of its *own* kind ends there, the
/// `break` written behind it lands outside the loop, and the loop runs forever
/// — and bash runs forever on the same shapes (measured rc 124 with the same
/// cap). These rows are pinned so a later "fix" cannot silently start
/// terminating them: the hang is the parity, not the bug. Capped children: rc
/// 124 at [`BOUND`] is the expected outcome. Sample of the rule, not its
/// extent — the shapes of the rule whose line has a **second** closer of the
/// block's own kind are [`a_loop_body_that_never_returns_where_bash_refuses_the_line`].
#[test]
fn a_loop_body_that_never_returns_is_bash_parity() {
    let rows = [
        "while :; do echo while; done; echo after",
        "while :; do echo if; done; echo after",
        "while :; do echo in; done; echo after",
        "while :; do echo fi; echo x; done; echo after",
        "while :; do if :; then echo if; fi; done; echo after",
        "while :; do if :; then :; fi done; echo after",
        "while :; do echo a \\| if; echo x; done; echo after",
    ];
    let codes = run_capped(&rows);
    for (script, code) in rows.iter().zip(codes) {
        assert_eq!(
            code, 124,
            "{script}: bash hangs here too (rc 124) — fdshell's hang is the parity"
        );
    }
}

/// Task #309, the divergence half of the same rule (README "body that never
/// returns"): the shape whose text keeps a **second** closer of the block's own
/// kind. The pairing ends the block at its own closer, the `break` behind it is
/// a top-level `break`, the loop body never returns — and bash escapes with rc 2
/// because it refuses the stray closer written after the loop (its grammar has
/// no such command), so the *cause* of the divergence is the acceptance of that
/// stray closer, not the pairing: the same acceptance
/// `tests/block_closers.rs::the_rc_1_family_stays_a_parse_error` pins at rc 1
/// for the shapes that do terminate. Filed as task **#436** (the acceptance
/// family, whose rule task **#405** owns). Capped children: rc 124 is the
/// measured outcome, and a row that starts terminating is a change to disclose.
#[test]
fn a_loop_body_that_never_returns_where_bash_refuses_the_line() {
    let rows = [
        "while :; do echo for; done; break; done; echo after",
        "while :; do echo until; done; break; done; echo after",
        "while :; do echo while; done; break; done; echo after",
        "while :; do echo fi; done; break; done; echo after",
        "while :; do echo esac; done; break; done; echo after",
        "while :; do echo if; fi; done; echo after",
        "while :; do echo if; done; esac; break; done; echo after",
        "if :; then while :; do echo b; fi; done; fi; echo after",
        "while :; do echo a \\| while :; do break; done; break; done; echo after",
    ];
    let codes = run_capped(&rows);
    for (script, code) in rows.iter().zip(codes) {
        assert_eq!(code, 124, "{script}: bash rc 2, fdshell runs forever");
    }
}

/// The `echo in done` twin of
/// [`a_loop_body_that_never_returns_is_bash_parity`]'s `echo in` row, healed by
/// task #301: `in` is `echo`'s
/// argument, `Place::Pattern` (the word-list place) is a rule the closer-shaped
/// word list of #273/#306/#310 owns, and the `done` written behind it scores as
/// a closer there — so the `while` closes at that `done` and the closer rule
/// rejects it: **rc 1** `expected ';' before 'done'` where
/// `master` ran forever (rc 124 at [`BOUND`]) and `bash -c` (5.3.9) prints
/// `in done` / `after` with rc 0. fdshell stays a divergence from bash (rc 1
/// vs 0), and the parity half belongs to the word-list tasks; the hang is gone.
#[test]
fn in_word_list_argument_before_a_closer_is_a_parse_error() {
    let (out, err, code) = run_bounded("while :; do echo in done; break; done; echo after");
    assert_eq!((out.as_str(), code), ("", 1), "stderr: {err}");
    assert!(err.contains("expected ';' before 'done'"), "stderr: {err}");
}

/// Task #292 moves these rows out of the hang family of
/// [`a_loop_body_that_never_returns_is_bash_parity`]: the `|` position is a
/// command position, so a keyword block behind a pipe *in a loop body* runs as
/// a forked pipeline stage and the loop's `break` lands again. Task #309 adds
/// the second half — an `if` block written behind a **glued** `|` closes at its
/// own `fi`, not at the enclosing loop's, so the two glued-`if` rows below now
/// match bash exactly (they were the rc-1 "glued pair" divergence, README).
/// The `while`-stage row keeps its rc 1 (bash rc 0): the glued pair's own
/// family; the two condition-less `if` rows are shapes bash itself rejects
/// (rc 2). On `master` every row of this list ran forever (rc 124 at [`BOUND`]).
#[test]
fn block_stage_in_a_loop_body_ends_the_loop() {
    for (script, want) in [
        (
            "while :; do echo a | if :; then echo y; fi; break; done; echo after",
            "y\nafter\n",
        ),
        (
            "while :; do echo a | case x in a) echo y;; esac; break; done; echo after",
            "after\n",
        ),
        // The glued `|` twins (#309's pairing): the `if`'s `fi` closes the
        // `if`, the loop's `break` lands, and the tail runs — bash's output.
        (
            "while :; do echo a|if :; then echo y; fi; break; done; echo after",
            "y\nafter\n",
        ),
        (
            "while :; do echo a|if :; then echo y; fi; echo z; break; done; echo after",
            "y\nz\nafter\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
    // The glued `|` with a `while` stage: parse error rc 1, bash rc 0 (the
    // glued-pair family, unchanged by #309's pairing).
    let script = "while :; do echo a|while :; do break; done; break; done; echo after";
    let (_out, _err, code) = run(script);
    assert_eq!(code, 1, "{script}: bash rc 0");
    // `if` with no condition, as a stage: bash rc 2, fdshell its own rc 1.
    for script in [
        "while :; do echo a | if; fi; echo x; break; done; echo after",
        "while :; do echo a | if; echo x; done; echo after",
    ] {
        let (_out, _err, code) = run(script);
        assert_eq!(code, 1, "{script}: bash rc 2");
    }
}

/// A plain `&` inside an `if` body is a command-list terminator (task #354): the
/// arms whose closing keyword follows the `&` on the same line parse, and the
/// **taken** arm's statement runs as a background job. The `else` keyword behind
/// the `&` is found, where master ran `else` as a command (`"else" not found`).
#[test]
fn ampersand_in_an_if_body_backgrounds_the_statement() {
    for (script, want) in [
        ("if true; then echo a & else echo b & fi", "a\n"),
        ("if false; then echo a & else echo b & fi", "b\n"),
        ("if true; then echo a & elif false; then echo c & fi", "a\n"),
        ("if false; then :; elif true; then echo e & fi", "e\n"),
    ] {
        let (out, err, rc) = run(&format!("{script}\nwait"));
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: {err}");
    }
    // A body glued to the closer keeps its `&`. The pins are the **job record**
    // and the job's own files, read from a scratch dir the child cannot write
    // into: the backgrounded child shares this process's stdout, so reading the
    // `$!` line out of stdout is a load flake (measured on the previous pin: 10
    // contaminated readings in 300 runs — `pid=1263737x` — and a real failure of
    // `cargo mutants`' unmutated baseline, LESSONS).
    let dir = scratch("if-body-pid");
    let (out, err, rc) = run_in(
        &dir,
        "if true; then builtin true & fi\nbuiltin echo $! >pid\nif true; then builtin echo x >body & fi\nwait",
    );
    assert_eq!((out.as_str(), rc), ("", 0), "{err}");
    let record = |name: &str| {
        std::fs::read_to_string(dir.join(name))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let pid: i64 = record("pid").parse().unwrap_or_default();
    assert!(pid > 0, "$! = {:?}, not a job pid", record("pid"));
    // The echoing variant, pinned as a job record too: the statement's output
    // went to its own file, so a background child never pollutes the parent's.
    assert_eq!(record("body"), "x", "the body statement ran as a job");
}

/// A `&`-terminated block **condition** runs as a background job (task #356):
/// the condition's side effects stay in the discarded child, the arm/loop it
/// gates sees the async status **0** — so `if false & then echo x` prints `x`
/// and `until false & do echo x; break; done` prints nothing, like bash — and
/// the whole AND-list behind it is the job (`if true && false & then echo x;
/// fi` prints `x`, bash's output). Every script ends with `wait` so the
/// background child closes the stdout pipe deterministically.
#[test]
fn ampersand_terminated_condition_parses_and_runs() {
    for (script, want) in [
        ("if true & then echo x; fi", "x\n"),
        ("if true & then echo x; else echo y; fi", "x\n"),
        ("if false & then echo x; else echo y; fi", "x\n"),
        ("while : & do echo x; break; done", "x\n"),
        ("while false & do echo x; break; done", "x\n"),
        ("until false & do echo x; break; done", ""),
        ("until true & do echo x; break; done", ""),
        ("if false; then :; elif true & then echo e; fi", "e\n"),
        (
            "if false; then :; elif false & then echo e; else echo l; fi",
            "e\n",
        ),
        ("if true &\nthen echo x; fi", "x\n"),
        ("if true &\n\nthen echo x; fi", "x\n"),
        ("while : &\ndo echo x; break; done", "x\n"),
        ("if true && false & then echo x; fi", "x\n"),
    ] {
        let (out, err, rc) = run(&format!("{script}\nwait"));
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: {err}");
    }
    // A condition whose last part is short-circuited away records no job and
    // gates on that part's status, where bash backgrounds the **whole** list
    // (measured divergence, owner #352): bash prints `x`, fdshell prints `f`.
    let (out, _err, rc) = run("if false && true & then echo x; else echo f; fi\nwait");
    assert_eq!((out.as_str(), rc), ("f\n", 0), "bash prints x: #352");
}

/// The **job record** of a `&`-terminated condition (task #356): `$!` is a
/// numeric pid for every shape. The pid is read from a **file** in a scratch
/// dir, never out of stdout — a backgrounded child shares this process's stdout
/// pipe, and reading `$!` from it is a measured load flake (the comment in
/// [`ampersand_in_an_if_body_backgrounds_the_statement`]: 10 contaminations in
/// 300 runs).
#[test]
fn ampersand_terminated_condition_records_a_job() {
    let dir = scratch("cond-job-pid");
    for (tag, script) in [
        ("if", "if true & then builtin true; fi"),
        ("while", "while : & do break; done"),
        ("until", "until false & do break; done"),
        (
            "elif",
            "if false; then :; elif true & then builtin true; fi",
        ),
        ("newline", "if true &\nthen builtin true; fi"),
        ("andlist", "if true && true & then builtin true; fi"),
    ] {
        let (out, err, rc) = run_in(&dir, &format!("{script}\nbuiltin echo $! >{tag}\nwait"));
        assert_eq!((out.as_str(), rc), ("", 0), "{script}: {err}");
        let pid: i64 = record(&dir, tag).trim().parse().unwrap_or_default();
        assert!(
            pid > 0,
            "{script}: $! = {:?}, not a job pid",
            record(&dir, tag)
        );
    }
    // The `& ;` shape records nothing: it stays the rc-1 parse error (task #355)
    // and the **preflight** bails, so the script writes no file at all.
    for script in [
        "if true & ; then builtin true; fi",
        "while : & ; do builtin true; break; done",
        "if false; then :; elif true & ; then builtin true; fi",
    ] {
        std::fs::remove_file(dir.join("amp")).ok();
        let (out, _err, rc) = run_in(&dir, &format!("{script}\nbuiltin echo $! >amp"));
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 2");
        assert!(
            record(&dir, "amp").is_empty(),
            "{script}: the script ran, $! moved"
        );
    }
}

/// The condition's side effects die with the child (task #356): a `cd` written
/// in a `&`-terminated condition leaves the parent's cwd where it was (bash
/// parity — master's foreground condition moved it).
#[test]
fn ampersand_terminated_condition_leaves_the_parent_cwd() {
    let dir = scratch("cond-cwd");
    std::fs::create_dir(dir.join("sub")).unwrap();
    let (out, err, rc) = run_in(
        &dir,
        "if cd sub & then builtin true; fi\nbuiltin pwd >cwd\nwait",
    );
    assert_eq!((out.as_str(), rc), ("", 0), "{err}");
    let got = record(&dir, "cwd");
    let got = got.trim();
    assert_eq!(got, dir.to_str().unwrap(), "the parent's cwd is untouched");
}

/// The `& ;` written **inside a block body** is rejected (task #355 fixed the
/// accepted divergence this pin used to record): the body slice **keeps** the
/// `;` glued behind the `&`, so the body's own scan marks the split invalid
/// and runs nothing — fdshell rc 1 where bash stops with rc 2 (the documented
/// rc-1-vs-2 family, README "Errors"). The taken arms and the condition slice
/// all reach that scan; an **untaken** arm's body is parsed, never scanned,
/// and stays the lazy-parse divergence pinned at
/// `tests/block_bodies.rs::ampersand_only_body_is_not_an_empty_body`.
#[test]
fn ampersand_then_semicolon_in_a_body_is_rejected() {
    for script in [
        "for i in 1; do echo a & ; done",
        "if true; then echo a & ; fi",
        "if false; then :; else echo a & ; fi",
        "if false; then :; elif true; then echo a & ; fi",
        "if true; then echo a & ; elif false; then :; fi",
        "if true & ; then echo x; fi",
        "case x in x) echo a & ; ;; esac",
        "f(){ echo a & ; }; f",
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 2: {err}");
        assert!(err.contains("unexpected character"), "{script}: {err}");
    }
    // The `while` twin puts the `;` **after** the `&` inside the body's own
    // statement list, so the body's scan flagged the invalid split before the
    // rule too: rc 1, bash rc 2 (the documented rc-1-vs-2 family).
    let (out, _err, rc) = run("while :; do echo a & ; break; done");
    assert_eq!((out.as_str(), rc), ("", 1), "bash rc 2");
    // The untaken arms parse, never scan: rc 0 with the `& ;` body that never
    // runs (bash rejects the shape at parse time, rc 2 — the lazy-parse
    // divergence of `block_bodies.rs`, now the family's owner).
    for script in [
        "if true; then :; else echo a & ; fi",
        "case y in x) echo a & ; ;; esac",
        "f(){ echo a & ; }",
    ] {
        let (out, _err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 0), "{script}: bash rc 2");
    }
}

/// Task #277 facet 2: an unclosed block is a statement-layer report, so it
/// names the construct and the keyword it wants, with the caret under the opener
/// (STYLE §4.7). The rc is the shell-wide 1 of a parse failure (bash: rc 2, the
/// documented rc-1-vs-2 family).
#[test]
fn unclosed_block_names_the_closer_it_wants() {
    for (script, want) in [
        ("if true; then echo a", "'if' is missing its closing 'fi'"),
        ("for i in a", "'for' is missing its closing 'done'"),
        ("while true", "'while' is missing its closing 'done'"),
        ("until false", "'until' is missing its closing 'done'"),
        ("case 1 in", "'case' is missing its closing 'esac'"),
        ("{ echo a", "'{' is missing its closing '}'"),
        ("f() {", "'f()' is missing its closing '}'"),
        ("function f", "'function' is missing its closing '}'"),
        // The `time` prefix is transparent: the keyword behind it is named.
        ("time while true", "'while' is missing its closing 'done'"),
    ] {
        let (out, err, rc) = run(script);
        assert_eq!((out.as_str(), rc), ("", 1), "{script}: bash rc 2");
        assert!(err.contains(want), "{script}: stderr={err}");
        // The `ParsePosition` attachment prints the line and its caret.
        assert!(err.contains(script), "{script}: stderr={err}");
    }
}

// Task #296 section: a word-start `#` comments out the rest of its **physical**
// line, so an `&&` / `||` written inside comment text is never a cond-list split
// point. A keyword block's span carries the closing keyword's whole run, comment
// included, which is what used to hand the comment's operator to the cond-list
// scan (the plain-statement twin was already inert: the segment scanner ends the
// statement at the `#`). Every row is measured against `bash` 5.3.9; the loop
// rows run under [`run_bounded`], because a comment that stops hiding its line
// would put the `break` behind a gate again.

/// The headline family: the comment behind a block's closing keyword (and inside
/// a mid-run block) hides its `&&` / `||`, so the comment's tail never runs.
#[test]
fn a_comment_after_a_block_hides_the_cond_operators() {
    for (script, want) in [
        ("while :; do echo a; break; done # c && echo y", "a\n"),
        ("if :; then echo t; fi # c && echo y", "t\n"),
        ("for i in 1; do echo $i; done # c && echo y", "1\n"),
        ("case x in x) echo c;; esac # c && echo y", "c\n"),
        ("until :; do echo u; break; done # c && echo y", ""),
        // The mid-run twins (task #232's span has the same run-end semantics).
        (
            "true && while :; do echo a; break; done # c && echo y",
            "a\n",
        ),
        ("true && if :; then echo t; fi # c && echo y", "t\n"),
        // The comment sits on the closer's own line of a multi-line block.
        ("while :; do\n  echo a\n  break\ndone # c && echo x", "a\n"),
        // A `;` behind the comment never resumes the list: `echo after` /
        // `echo tail` are comment text too, and `set -e` sees the block's own
        // status.
        (
            "while :; do echo a; break; done # c && echo y; echo after",
            "a\n",
        ),
        (
            "set -e; while :; do echo a; break; done # c && echo y; echo after",
            "a\n",
        ),
        ("if :; then echo t; fi # c && echo y; echo tail", "t\n"),
        // A closer or a brace named in the comment is comment text: it closes
        // no block, opens no group, and its operator stays hidden.
        ("while :; do echo a; break; done # done && echo y", "a\n"),
        ("while :; do echo a; break; done # { && echo y", "a\n"),
    ] {
        let (out, err, rc) = run_bounded(script);
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: stderr={err}");
    }
}

/// The gate scan shares the rule: a short-circuited `||` gates bash's AND-list,
/// and a `;` written **inside** the comment never ends the gate, so
/// `run_cond_list` never resumes ungated inside comment text (it used to run
/// `echo y` / `echo tail` there).
#[test]
fn a_comment_terminates_a_short_circuit_gate() {
    for (script, want) in [
        ("true || while :; do :; done # c ; echo y", ""),
        ("true || if :; then :; fi # fi ; echo tail", ""),
        ("echo a || if :; then :; fi # done ; echo tail", "a\n"),
    ] {
        let (out, err, rc) = run_bounded(script);
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: stderr={err}");
    }
    // The comment's own **newline** is the gate's end: the statement written on
    // the next line runs ungated, exactly as a plain newline ends a gate.
    let (out, err, rc) = run_bounded("true || if :; then :; fi # c\necho tail");
    assert_eq!((out.as_str(), rc), ("tail\n", 0), "stderr={err}");
}

/// Non-regression: the comment shapes that already matched bash stay
/// byte-identical. A plain statement ends at the `#` (the segment scanner), a
/// `{ …; }` group ends there too, and a comment tail after a block is inert
/// through the parse layer, which stops at a comment inside a part's text — the
/// dependency task #296 relies on (a `;` in the comment never resumes the list).
/// The last row is the shape task **#333** moved from divergence to parity: a
/// `#`-terminated line keeps the newline's end-of-line bookkeeping, so the
/// here-doc owns its region and the `&&` written in that region is data.
#[test]
fn a_comment_tail_next_to_a_block_stays_inert() {
    for (script, want) in [
        ("echo a # c && echo y", "a\n"),
        ("true && { echo g; } # c && echo y", "g\n"),
        ("while :; do echo a; break; done # c ; echo after", "a\n"),
        (
            "while :; do echo a; break; done # c\necho tail",
            "a\ntail\n",
        ),
        ("while :; do echo a; break; done # c | cat", "a\n"),
        ("while :; do echo a; break; done # c & echo y", "a\n"),
        // The word-start half of the rule: a `#` inside a word is data.
        ("echo a x#y && echo b", "a x#y\nb\n"),
        // Task #333: the comment ends the line, so the region starts at the
        // comment's own newline and its `&&` is body data, never a split point.
        ("if :; then cat <<Q # c\nq && y\nQ\nfi", "q && y\n"),
    ] {
        let (out, err, rc) = run_bounded(script);
        assert_eq!((out.as_str(), rc), (want, 0), "{script}: stderr={err}");
    }
}

/// The comment edges bash rejects and fdshell does not (yet) are pinned with
/// their **current** result, so this fix is measured as not touching them; the
/// owner task is named per row (the rc-1-vs-2 family, README "Accepted
/// divergences").
#[test]
fn a_comment_at_a_block_edge_keeps_the_filed_divergences() {
    // #405: a word written after the closing keyword. The glued closer `done#c`
    // never closes, so fdshell reports the unclosed block (bash: rc 2).
    let (out, err, rc) = run_bounded("while :; do echo a; break; done#c && echo y");
    assert_eq!((out.as_str(), rc), ("", 1), "stderr={err}");
    assert!(
        err.contains("'while' is missing its closing 'done'"),
        "stderr={err}"
    );
    // #405: a quoted word behind the closer is accepted and the block runs. The
    // comment's `&& echo y` is hidden by task #296, so only the block's own text
    // is left; the residual divergence is the acceptance of the word.
    let (out, err, rc) = run_bounded("while :; do echo a; break; done \"c\" # && echo y");
    assert_eq!((out.as_str(), rc), ("a\n", 0), "stderr={err}");
    // #405: a word glued with `#` behind `fi` is an argument, and the `&&` runs
    // the tail (bash: rc 2 at the word).
    let (out, err, rc) = run_bounded("if :; then echo t; fi x#y && echo y");
    assert_eq!((out.as_str(), rc), ("t\ny\n", 0), "stderr={err}");
    // #406: a `&&` at the start of a physical line is an operator where bash
    // stops with rc 2.
    let (out, err, rc) = run_bounded("echo a # c\n&& echo y");
    assert_eq!((out.as_str(), rc), ("a\ny\n", 0), "stderr={err}");
    // #404: a `#` line inside a `{ …; }` body breaks the group parse.
    let (out, err, rc) = run_bounded("{ echo g # c && echo z\n} && echo w");
    assert_eq!((out.as_str(), rc), ("", 1), "stderr={err}");
    assert!(err.contains("expected ';' before '}'"), "stderr={err}");
    // #304 closed this row's **region-rule** half (a region claimed by a word of
    // the block's body is opaque, so the `&&` in `q && y` never splits the block
    // text); the **statement-level** half — a `#` behind a here-doc delimiter
    // word — reached parity in task **#333** and is pinned in
    // `a_comment_tail_next_to_a_block_stays_inert`.
    // #304's twin, where fdshell runs the statement: the `X` delimiter word ends
    // the heredoc and `X` is then run as a command, so only the body's `b` and
    // the report's rc are machine-independent (bash: rc 2 at `fi`).
    let (out, _, rc) = run_bounded("if cat <<X; then # c\nb\nX\nfi");
    assert_eq!((out.strip_prefix("b\n").is_some(), rc), (true, 1));
}

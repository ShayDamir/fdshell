#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Glued cond-pair and short-circuit-gate pins (task #236): the end-to-end
//! half of the pair rule (the in-process half is pinned in
//! `keywords/position/tests.rs`, `keywords/pair/tests.rs`,
//! `cond/boundary/block/tests.rs`, `parse/semi/closing/tests.rs` and
//! `cond/tests.rs`). The shapes are measured against bash 5.3.9 and run
//! against `CARGO_BIN_EXE_fdshell` unconditionally: the dev and release
//! binaries compute these shapes identically (LESSONS, task #236 — the only
//! profile difference is the source-location frames of a debug error report),
//! so there is no profile to dodge and no skip path.

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// Run with a bounded child (the loop rows must end on their own; a hung
/// child is killed and the shape fails with rc 124 — same harness as
/// `tests/cond_blocks.rs::run_bounded`).
fn run_bounded(script: &str) -> (String, String, i32) {
    let mut child = Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn fdshell");
    let started = std::time::Instant::now();
    while child.try_wait().expect("try_wait").is_none() {
        if started.elapsed() > std::time::Duration::from_secs(5) {
            let _ = Command::new("kill")
                .args(["-KILL", &child.id().to_string()])
                .status();
            let _ = child.wait();
            return (String::new(), String::new(), 124);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let output = child.wait_with_output().expect("wait_with_output");
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Task #236 (S2): a glued `&&`/`||` pair *keeps* the command position the
/// pair sets, so the keyword glued behind it is scored: `x||if :; …; fi`
/// nests the `if` block (its `;`s are clause bytes) and the statement runs
/// like bash's. The pipeline `|` twin is scored the same way since task
/// **#292** (the `|` sets a command position for the *next stage*), so all
/// three glue shapes are bash parity.
#[test]
fn glued_pair_keeps_the_command_position_the_pair_sets() {
    // The spaced pairs: bash parity — the `&&` twin runs the `if` (prints
    // `y`), the `||` twin short-circuits it.
    let (out, err, code) = run_bounded("echo x&&if :; then echo y; fi; echo after");
    assert_eq!((out.as_str(), code), ("x\ny\nafter\n", 0), "stderr={err}");
    let (out, err, code) = run_bounded("echo x||if :; then echo y; fi; echo after");
    assert_eq!((out.as_str(), code), ("x\nafter\n", 0), "stderr={err}");
    // The pipeline `|` twin, #292: the `if` behind the pipeline byte *is*
    // scored, it opens the second stage's block (its `;`s are clause bytes)
    // and the stage runs in the fork — bash prints `y`, `after`.
    let (out, err, code) = run_bounded("echo x|if :; then echo y; fi; echo after");
    assert_eq!((out.as_str(), code), ("y\nafter\n", 0), "stderr={err}");
}

/// Task #236 (S2): a short-circuited `||` chain gates the AND-list and the
/// *statement* behind it, not the rest of the line — the scan skips the
/// gated text (a keyword block the pair's payload opened, closed at its own
/// closer, an opaque `{ …; }` group) and resumes ungated past the `;` that
/// ends it, so `break`/`echo` behind the block run.
#[test]
fn short_circuit_gate_skips_the_gated_list_and_resumes_behind_it() {
    // The gate skips the `if` block and `echo tail` runs (bash parity).
    let (out, err, code) = run_bounded("echo a || if :; then :; fi; echo tail");
    assert_eq!((out.as_str(), code), ("a\ntail\n", 0), "stderr={err}");
    // The glued pair: the payload `if` opens the block the gate skips, its
    // `fi` closes it, and `echo tail` runs (bash parity, S2).
    let (out, err, code) = run_bounded("echo a||if :; then :; fi; echo tail");
    assert_eq!((out.as_str(), code), ("a\ntail\n", 0), "stderr={err}");
    // The gate's `;` is the *first* depth-0 separator behind the operator.
    let (out, err, code) = run_bounded("echo a || :; echo b");
    assert_eq!((out.as_str(), code), ("a\nb\n", 0), "stderr={err}");
    // The S2 row: the gated `if` inside a loop body is skipped, `break`
    // lands, the loop ends, `echo after` runs (bash parity).
    let (out, err, code) =
        run_bounded("while :; do echo a || if :; then :; fi; break; done; echo after");
    assert_eq!((out.as_str(), code), ("a\nafter\n", 0), "stderr={err}");
    // Its glued twins: the *in-block* glued pair is the measured accepted
    // divergence — the parse layer's token walk keeps the glued `if` in the
    // operand word's shadow, the block span breaks and the statement dies
    // with a parse error (rc 1, master parity; bash runs it, printing
    // `a`, `after`).
    let (out, err, code) =
        run_bounded("while :; do echo a&&if :; then :; fi; break; done; echo after");
    assert_eq!(
        (out.as_str(), code),
        ("a\n", 1),
        "stderr={err}: master parity"
    );
    let (out, err, code) =
        run_bounded("while :; do echo a||if :; then :; fi; break; done; echo after");
    assert_eq!((out.as_str(), code), ("", 1), "stderr={err}: master parity");
    // A `{ …; }` group is opaque to the gate and its `;` ends it: fdshell
    // prints `a` (the `&&` tail runs after the gate) — master parity, an
    // accepted divergence (bash prints only `a`; measured).
    let (out, err, code) = run_bounded("echo a || { echo b; } && echo c");
    assert_eq!((out.as_str(), code), ("a\n", 0), "stderr={err}");
}

/// Task #326: the gate's window starts **right behind the pair**, so it never
/// reaches back over the pair's own bytes. A **one-byte** left operand glued
/// to the pair (`:`, the shell's own command) puts the operator at offset 1 —
/// the row that kills `cond.rs`'s `let after = i - 2` mutant: in the dev
/// profile (what nextest and `cargo mutants` build) the back-step underflows
/// and the shell dies with rc 101, and in release the wrapped index finds no
/// separator so the gate ends at the same byte (the mutant is equivalent
/// there, so the row pins the *shape* and the profile does the killing —
/// LESSONS, task #326). All rows measured against bash 5.3.9: identical.
#[test]
fn one_byte_operand_glued_to_the_pair_keeps_the_gate_behind_the_pair() {
    let (out, err, code) = run_bounded(":||echo b");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    let (out, err, code) = run_bounded(":||echo b; echo after");
    assert_eq!((out.as_str(), code), ("after\n", 0), "stderr={err}");
    // Its twins: the `&&` pair runs the tail, the spaced `||` short-circuits
    // to the `;` exactly like the glued form.
    let (out, err, code) = run_bounded(":&&echo b");
    assert_eq!((out.as_str(), code), ("b\n", 0), "stderr={err}");
    let (out, err, code) = run_bounded(": || echo b; echo after");
    assert_eq!((out.as_str(), code), ("after\n", 0), "stderr={err}");
}

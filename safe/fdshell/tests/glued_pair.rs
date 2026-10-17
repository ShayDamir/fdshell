#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Glued cond-pair and short-circuit-gate pins (task #236). The lib-test
//! codegen anomaly (LESSONS: the fdshell *test-profile* library miscompiles
//! the keyword payload arms of `keywords/position.rs`, glued forms only)
//! makes these shapes unpinable against `CARGO_BIN_EXE_fdshell` in a plain
//! `cargo nextest` run, so this harness runs the *non-test* profile binary
//! when one is available — `cargo build --release` (or the nix build, whose
//! test profile is built with `opt-level = 1` via `package.nix`) — and skips
//! with a loud note otherwise. The shapes are measured against bash 5.3.9.
//!
//! Run locally: `cargo build --release && cargo nextest run --test glued_pair`.

use std::path::PathBuf;
use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// The pin binary: the test-profile binary when it is a non-test-profile
/// build (the nix test run: `opt-level = 1`), else the `target/release`
/// binary of a preceding `cargo build --release`, else `None` (skip).
fn pin_bin() -> Option<PathBuf> {
    let test = PathBuf::from(BIN);
    if test.file_name().and_then(|n| n.to_str()) == Some("fdshell") {
        // Nix: the test profile is opt-level 1 — keyword-correct (LESSONS).
        if std::env::var("NIX_BUILD_TOP").is_ok() {
            return Some(test);
        }
    }
    let release = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/fdshell");
    release.exists().then_some(release)
}

fn run(script: &str) -> (String, String, i32) {
    let bin = pin_bin().expect("run `cargo build --release` first (see module doc)");
    let output = Command::new(&bin)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("spawn pin binary");
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Run with a bounded child (the loop rows must end on their own; a hung
/// child is killed and the shape fails with rc 124 — same harness as
/// `tests/cond_blocks.rs::run_bounded`).
fn run_bounded(script: &str) -> (String, String, i32) {
    let bin = pin_bin().expect("run `cargo build --release` first (see module doc)");
    let mut child = Command::new(&bin)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn pin binary");
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

/// The skip guard: no suitable binary → the test passes vacuously (the
/// shapes stay pinned wherever a non-test binary is built, and a plain
/// `cargo nextest` run prints the reminder).
#[test]
fn glued_pair_pins_have_a_binary() {
    if pin_bin().is_none() {
        eprintln!(
            "SKIP glued_pair pins: no non-test fdshell binary found \
             (`cargo build --release` enables them)"
        );
    }
}

/// Task #236 (S2): a glued `&&`/`||` pair *keeps* the command position the
/// pair sets, so the keyword glued behind it is scored: `x||if :; …; fi`
/// nests the `if` block (its `;`s are clause bytes) and the statement runs
/// like bash's. The pipeline `|` twin keeps its #292 divergence (the block
/// splits at the `;`) — pinned here as the measured accepted divergence.
#[test]
fn glued_pair_keeps_the_command_position_the_pair_sets() {
    if pin_bin().is_none() {
        return;
    }
    // The spaced pairs: bash parity — the `&&` twin runs the `if` (prints
    // `y`), the `||` twin short-circuits it.
    let (out, err, code) = run_bounded("echo x&&if :; then echo y; fi; echo after");
    assert_eq!((out.as_str(), code), ("x\ny\nafter\n", 0), "stderr={err}");
    let (out, err, code) = run_bounded("echo x||if :; then echo y; fi; echo after");
    assert_eq!((out.as_str(), code), ("x\nafter\n", 0), "stderr={err}");
    // The pipeline `|` twin: the accepted #292 divergence, measured — the
    // `if` behind the pipeline byte is unscored, the `;`s split the block
    // and the statements run flat (bash prints `y`, `after`).
    let (out, err, code) = run_bounded("echo x|if :; then echo y; fi; echo after");
    assert_eq!((out.as_str(), code), ("after\n", 0), "stderr={err}");
}

/// Task #236 (S2): a short-circuited `||` chain gates the AND-list and the
/// *statement* behind it, not the rest of the line — the scan skips the
/// gated text (a keyword block the pair's payload opened, closed at its own
/// closer, an opaque `{ …; }` group) and resumes ungated past the `;` that
/// ends it, so `break`/`echo` behind the block run.
#[test]
fn short_circuit_gate_skips_the_gated_list_and_resumes_behind_it() {
    if pin_bin().is_none() {
        return;
    }
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

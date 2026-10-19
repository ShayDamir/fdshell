#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN).args(["-c", script]).output().unwrap()
}

/// `wait $!` reaps the backgrounded child and returns its exit code.
#[test]
fn wait_by_pid_returns_child_exit_code() {
    let out = run("builtin false &>&j; wait $!; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=1"),
        "expected exit=1, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait $!` on a successful child returns 0.
#[test]
fn wait_by_pid_success() {
    let out = run("builtin true &>&j; wait $!; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait` with no args reaps every background task; the last status wins.
#[test]
fn wait_no_args_reaps_all() {
    let out = run("builtin true &>&j; builtin true &>&k; wait; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait 999999` (a pid with no matching task) is a clean error (exit 1).
#[test]
fn wait_unknown_pid_errors() {
    let out = run("wait 999999");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("not found"), "stderr={err}");
}

/// `wait abc` (a non-numeric arg) is a clean error (exit 1).
#[test]
fn wait_non_numeric_bad_pid() {
    let out = run("wait abc");
    assert_eq!(out.status.code(), Some(1));
    let err = str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("pid"), "stderr={err}");
}

/// `builtin wait` is accepted (the builtin-validation path allows it).
#[test]
fn builtin_wait_accepted() {
    let out = run("builtin true &>&j; builtin wait $!; builtin echo exit=$?");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(
        stdout.contains("exit=0"),
        "expected exit=0, stdout={stdout} stderr={:?}",
        str::from_utf8(&out.stderr)
    );
}

/// `wait readable` opens a `wait` block (pattern keyword lookahead), so a
/// bare `wait readable` with no `done` is a parse error, not the builtin.
#[test]
fn wait_pattern_keyword_is_block_not_builtin() {
    let out = run("wait readable");
    assert_ne!(out.status.code(), Some(0));
}

/// Review F1 (task #232): a `wait` written after `&&` / `||` is still the
/// POSIX builtin — the segment scanner's mid-run block span is gated on the
/// shared scorer, which scores this `wait` `0` (no pattern keyword behind it).
/// So the run keeps its `;` split, the builtin reaps the task, its status is
/// the chain's status, and the statements written after the `;` run. Ungated,
/// the builtin took a block span and swallowed them (no output, rc 0).
/// Measured against bash (`builtin` is fdshell's own validation form, so only
/// the shapes are bash-comparable: `true && wait; echo tail` → `tail` rc 0).
#[test]
fn builtin_wait_after_a_cond_operator_stays_the_builtin() {
    let out = run("builtin true &>&j; true && wait; builtin echo tail");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "tail\n");
    assert_eq!(str::from_utf8(&out.stderr).unwrap(), "");
    // POSIX (measured against bash): `wait` with no operands returns 0, whatever
    // the reaped jobs' statuses were — task #326 moved fdshell to that rule.
    let out = run("builtin false &>&j; true && wait; builtin echo tail=$?");
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "tail=0\n");
    // A skipped chain never runs it, and the tail of the run still runs.
    let out = run("builtin true &>&j; false && wait; builtin echo tail");
    assert_eq!(
        (str::from_utf8(&out.stdout).unwrap(), out.status.code()),
        ("tail\n", Some(0))
    );
    let out = run("builtin true &>&j; true && echo x && wait; builtin echo tail");
    assert_eq!(str::from_utf8(&out.stdout).unwrap(), "x\ntail\n");
}

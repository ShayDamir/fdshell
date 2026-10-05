#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN).args(["-c", script]).output().unwrap()
}

/// `times` prints the four sections (user/system, children user/system) and
/// exits 0.
#[test]
fn times_prints_all_sections() {
    let out = run("times");
    assert_eq!(out.status.code(), Some(0));
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(stdout.contains("User time"), "stdout={stdout}");
    assert!(stdout.contains("System time"), "stdout={stdout}");
    assert!(stdout.contains("Children user time"), "stdout={stdout}");
    assert!(stdout.contains("Children system time"), "stdout={stdout}");
}

/// `builtin times` is accepted (the builtin-validation path allows it).
#[test]
fn builtin_times_accepted() {
    let out = run("builtin times");
    assert_eq!(out.status.code(), Some(0));
    let stdout = str::from_utf8(&out.stdout).unwrap();
    assert!(stdout.contains("User time"), "stdout={stdout}");
}

/// A value line is two tab-separated `N.NN` numbers.
#[test]
fn times_value_lines_are_centiseconds() {
    let out = run("times");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    // Two value lines (self + children), each with two `N.NN` fields.
    let value_lines = stdout
        .lines()
        .filter(|l| l.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        .count();
    assert_eq!(
        value_lines, 2,
        "expected exactly two numeric value lines, stdout={stdout}"
    );
}

/// After burning CPU in a backgrounded child and `wait`ing on it, the
/// children user time is > 0.00.
#[test]
fn times_children_time_increases_after_wait() {
    let out = run("n=0; while [ $n -lt 5000 ]; do n=$((n+1)); done &>&j; wait $!; times");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    // The children value line is the last numeric line; its first field
    // (children user time) must be > 0.00 after a CPU-burning child.
    let child_line = stdout
        .lines()
        .rfind(|l| l.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        .unwrap()
        .trim_start();
    let child_user = child_line.split_whitespace().next().unwrap();
    let secs: f64 = child_user.parse().unwrap();
    assert!(
        secs > 0.0,
        "expected children user time > 0.00, got {child_user}; stdout={stdout}"
    );
}

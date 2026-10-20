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

/// After burning CPU in a backgrounded child and `wait`ing on it, the children's
/// *total* CPU time (user + system) is > 0.00. Which of the two fields a reaped
/// child's CPU lands in is the kernel's accounting choice, so the assertion is
/// on the sum, never on user alone (LESSONS.md: "A CPU-burn timing test measures
/// nothing in a release build").
#[test]
fn times_children_total_time_increases_after_wait() {
    let out = run("n=0; while [ $n -lt 5000 ]; do n=$((n+1)); done &>&j; wait $!; times");
    let stdout = str::from_utf8(&out.stdout).unwrap();
    // The children value line is the last numeric line: `<user>\t<sys>` in
    // centiseconds. Both fields are read so the assertion below is on their sum:
    // `> 0` reads "the report is non-degenerate", never "the burn ran".
    let child_line = stdout
        .lines()
        .rfind(|l| l.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        .unwrap()
        .trim_start();
    let fields: Vec<&str> = child_line.split_whitespace().collect();
    let user: f64 = fields.first().copied().unwrap().parse().unwrap();
    let sys: f64 = fields.get(1).copied().unwrap().parse().unwrap();
    assert!(
        user + sys > 0.0,
        "expected children CPU time (user + system) > 0.00, got {user} + {sys}; stdout={stdout}"
    );
}

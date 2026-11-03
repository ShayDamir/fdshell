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

/// After burning CPU in the loop's forked children and reaping a backgrounded task
/// by name, the children's *total* CPU time (user + system) is > 0.00. Which of the
/// two fields a reaped
/// child's CPU lands in is the kernel's accounting choice, so the assertion is
/// on the sum, never on user alone (LESSONS: "A CPU-burn timing test measures
/// nothing in a release build").
///
/// The `&>&j` capture sits on a **simple command** (`builtin true`), which is where
/// the family's pidvar spelling lives: written behind a block's closing keyword
/// (`… done &>&j`, the shape this test used before task #409) bash rejects the line
/// and fdshell reports its parse error, so nothing here would run (POSIX_COMPATIBILITY
/// "`&` names background tasks").
///
/// The loop bound is sized by two floors, not by the assertion (any non-zero sum
/// passes it): the `centis` print floor — one field prints `0.00` below 100 µs,
/// the printed sum only below ~200 µs — and the nextest `slow-timeout` margin,
/// since a `slow-timeout` kill under a loaded `cargo mutants` sweep is a *false*
/// kill of every mutant in the copy (LESSONS: "A mutant caught by a flaky test
/// is a false kill"; the load figures are in the #267 bullet of "A CPU-burn timing
/// test measures nothing in a release build"). 500 iterations, 10 runs per
/// profile, children line in centiseconds, measured 2026-11-03 on this spelling
/// (debug `6.09`–`6.58` user + `0.70`–`1.08` system, release `4.84`–`5.17` user +
/// `0.39`–`0.66` system —
/// worst field `0.39` = 3900 µs, 39× the per-field print floor; the pre-#409
/// spelling, with the burn's CPU landing in the same line, measured
/// `5.65`–`7.19` + `0.73`–`1.36` debug and `3.73`–`4.24` + `0.28`–`0.50` release). The
/// 5000-iteration burn this replaces needed 25.8–26.4 s of wall in 8 concurrent
/// suite runs and so died at the 10 s period; `.config/nextest.toml` widens this
/// test's kill, never the period.
#[test]
fn times_children_total_time_increases_after_wait() {
    let out =
        run("n=0; while [ $n -lt 500 ]; do n=$((n+1)); done; builtin true &>&j; waitpid &j; times");
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

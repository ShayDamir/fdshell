#![cfg_attr(test, allow(clippy::unwrap_used))]
#![allow(clippy::indexing_slicing)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN).args(["-c", script]).output().unwrap()
}

fn stdout(out: &std::process::Output) -> String {
    str::from_utf8(&out.stdout).unwrap().to_string()
}

fn stderr(out: &std::process::Output) -> String {
    str::from_utf8(&out.stderr).unwrap().to_string()
}

/// The report's `name value` fields, as `(name, value)` pairs.
fn fields(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (name, value) = line.split_once([' ', '\t'])?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .filter(|(name, _)| matches!(name.as_str(), "real" | "user" | "sys"))
        .collect()
}

/// Parse a `N.NN` seconds field.
fn seconds(value: &str) -> f64 {
    value.parse::<f64>().unwrap()
}

/// Parse a default-layout `N m N.NNN s` field.
fn clock_secs(value: &str) -> f64 {
    let (mins, rest) = value.split_once('m').unwrap();
    seconds(&mins.replace('s', "")) * 60.0 + seconds(rest.trim_end_matches('s'))
}

/// The report is written on stderr; the inner's stdout stays clean.
#[test]
fn report_goes_to_stderr_and_not_stdout() {
    let out = run("time sleep 0.2");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout(&out), "");
    let report = fields(&stderr(&out));
    assert_eq!(report.len(), 3, "stderr={}", stderr(&out));
    assert_eq!(report[0].0, "real");
    assert_eq!(report[1].0, "user");
    assert_eq!(report[2].0, "sys");
    assert!(
        clock_secs(&report[0].1) >= 0.15,
        "real must cover the sleep: {}",
        report[0].1
    );
}

/// `-p` selects the POSIX layout: three `name value` lines, no leading blank
/// line, values in seconds.
#[test]
fn dash_p_reports_posix_seconds() {
    let out = run("time -p sleep 1");
    assert_eq!(out.status.code(), Some(0));
    let report = stderr(&out);
    assert!(
        report.starts_with("real "),
        "no leading blank line in the -p layout: {report}"
    );
    let lines: Vec<&str> = report.trim_end().lines().collect();
    assert_eq!(lines.len(), 3, "stderr={report}");
    for (name, i) in [("real", 0), ("user", 1), ("sys", 2)] {
        assert!(lines[i].starts_with(name), "line {}: {}", i, lines[i]);
    }
    assert!(
        seconds(&fields(&report)[0].1) >= 1.0,
        "real must cover the sleep: {}",
        fields(&report)[0].1
    );
}

/// The default layout is bash's: a leading blank line, tab-separated
/// `N m N.NNN s` fields.
#[test]
fn default_layout_is_bashs_clock_format() {
    let out = run("time sleep 0.2");
    let report = stderr(&out);
    assert!(report.starts_with("\nreal\t"), "stderr={report:?}");
    assert!(report.contains("\nuser\t"));
    assert!(report.contains("\nsys\t"));
    for (_, value) in fields(&report) {
        assert!(
            value.ends_with('s') && value.contains('m'),
            "clock layout: {value}"
        );
    }
}

/// The inner's exit status is the statement's status; the report never changes it.
#[test]
fn inner_status_is_the_statements_status() {
    let out = run("time false; echo rc=$?");
    assert!(stdout(&out).contains("rc=1"), "stdout={}", stdout(&out));
    let out = run("time ls /nope_dir_x; echo rc=$?");
    assert!(stdout(&out).contains("rc=2"), "stdout={}", stdout(&out));
    let out = run("time true && echo ok");
    assert!(stdout(&out).contains("ok"), "stdout={}", stdout(&out));
}

/// `exit` and `exec` leave (or replace) the shell, so their report is printed
/// before they run; the status is the `exit` code.
#[test]
fn terminating_commands_report_before_leaving() {
    for (script, code) in [
        ("time exit 3", 3),
        ("time -p exit 4", 4),
        ("time quit 2", 2),
    ] {
        let out = run(script);
        assert_eq!(out.status.code(), Some(code), "{script}");
        assert_eq!(fields(&stderr(&out)).len(), 3, "{script}: {}", stderr(&out));
    }
}

/// A bare `time` (with and without `-p`) reports zeros and succeeds.
#[test]
fn bare_time_reports_zeros() {
    for script in ["time", "time -p"] {
        let out = run(script);
        assert_eq!(out.status.code(), Some(0), "{script}");
        let report = fields(&stderr(&out));
        assert_eq!(report.len(), 3, "{script}: {}", stderr(&out));
        for (_, value) in &report {
            let raw = value.replace(['m', 's'], "");
            assert_eq!(raw.parse::<f64>().unwrap(), 0.0, "{script}: {value}");
        }
    }
}

/// Every leading `time [-p]` collapses: one statement, one report.
#[test]
fn repeated_prefixes_print_one_report() {
    for script in [
        "time time ls /dev/null",
        "time -p time ls /dev/null",
        "time time -p time ls /dev/null",
    ] {
        let out = run(script);
        assert_eq!(out.status.code(), Some(0), "{script}");
        assert_eq!(fields(&stderr(&out)).len(), 3, "{script}: {}", stderr(&out));
    }
}

/// Keyword blocks are timed whole; the block's own output is untouched.
#[test]
fn keyword_blocks_are_timed() {
    let out = run("time if true; then echo yes; fi");
    assert!(stdout(&out).contains("yes"), "stdout={}", stdout(&out));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));

    let out = run("time for i in 1 2; do echo $i; done");
    assert_eq!(stdout(&out), "1\n2\n", "stdout={}", stdout(&out));
    assert_eq!(out.status.code(), Some(0));

    let out = run("time while false; do echo x; done");
    assert_eq!(stdout(&out), "");
    assert_eq!(fields(&stderr(&out)).len(), 3);

    let out = run("time case x in x) echo X;; esac");
    assert!(stdout(&out).contains("X"));

    let out = run("time until false; do break; done");
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
}

/// A `{ …; }` group with a prefix is timed, and so is a group in a cond part.
#[test]
fn group_command_is_timed() {
    let out = run("time { echo a; echo b; }");
    assert_eq!(stdout(&out), "a\nb\n", "stdout={}", stdout(&out));
    assert_eq!(fields(&stderr(&out)).len(), 3);

    let out = run("true && time { echo c; }");
    assert!(stdout(&out).contains("c"), "stdout={}", stdout(&out));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
}

/// A multi-line block is timed whole.
#[test]
fn multi_line_block_is_timed() {
    let out = run("time if true; then\n echo yes\n fi");
    assert!(stdout(&out).contains("yes"), "stdout={}", stdout(&out));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
}

/// Is the host's `time` utility runnable (`Command` resolves it as a file, so
/// a shell's own keyword is not found here). The tests that compare against the
/// utility's output need it; a sandbox without it only checks that fdshell
/// prints no reserved-word report.
fn time_utility() -> bool {
    Command::new("time")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

/// A pipeline is timed whole; `time` on a pipeline stage is not a reserved
/// word (`time` is only one at a command position), so no report of ours is
/// printed — the host's `time` utility prints its own layout when installed.
#[test]
fn pipeline_is_timed_whole_and_stage_is_not_a_reserved_word() {
    let out = run("time sleep 0.2 | cat");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));

    let out = run("sleep 0.2 | time cat");
    let report = stderr(&out);
    assert_eq!(fields(&report), vec![], "our layout in a stage: {report}");
    if time_utility() {
        assert!(report.contains("elapsed"), "external time layout: {report}");
    }
}

/// A `time` inside a block body prints its own report, after the outer one.
#[test]
fn nested_time_in_a_block_prints_two_reports() {
    let out = run("time if true; then time sleep 0.1; fi");
    let report = stderr(&out);
    let real = report
        .lines()
        .filter(|l| l.trim_start().starts_with("real"))
        .count();
    assert_eq!(real, 2, "stderr={report}");
}

/// A function call is timed.
#[test]
fn function_call_is_timed() {
    let out = run("f(){ echo in f; }; time f");
    assert!(stdout(&out).contains("in f"), "stdout={}", stdout(&out));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
}

/// A heredoc in the timed statement still feeds the command its body.
#[test]
fn heredoc_in_timed_statement_keeps_its_body() {
    let out = run("time cat <<EOF\nhello\nEOF");
    assert!(stdout(&out).contains("hello"), "stdout={}", stdout(&out));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

/// Quoting any character of `time` makes it the utility, not the reserved word
/// (XCU 2.4): the host's `/usr/bin/time` runs it.
#[test]
fn quoted_time_is_the_utility() {
    for script in [
        // Single quotes keep their bytes in fdshell today (a pre-existing gap,
        // tracked separately), so only double-quoted forms are exercised.
        "\"time\" ls /dev/null",
        "t\"i\"me ls /dev/null",
    ] {
        let out = run(script);
        let report = stderr(&out);
        assert_eq!(
            fields(&report),
            vec![],
            "{script}: reserved-word layout in {report}"
        );
        if time_utility() {
            assert!(report.contains("elapsed"), "{script}: {report}");
        }
    }
}

/// A `time` alias is not expanded: `time` is in the reserved-word list the
/// alias expander skips, exactly like `if`/`while`/`done`, so the reserved word
/// runs. (bash expands aliases before recognizing keywords; fdshell keeps its
/// established reserved-word rule for all of them.)
#[test]
fn time_alias_is_not_expanded() {
    let out = run("alias time=echo; time ls /dev/null");
    assert_eq!(stdout(&out), "/dev/null\n", "stdout={}", stdout(&out));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
}

/// `set -x` reports are emitted for the timed statement; the report is still
/// printed once, on stderr.
#[test]
fn report_survives_xtrace() {
    let out = run("set -x; time sleep 0.1");
    let report = stderr(&out);
    assert!(report.contains("+ sleep 0.1"), "stderr={report}");
    assert_eq!(fields(&report).len(), 3, "stderr={report}");
}

/// Under `set -e` a timed statement behaves like any other: a successful
/// `time … && …` continues, and the report is printed.
#[test]
fn errexit_keeps_working() {
    let out = run("set -e; time sleep 0.1 && echo ok; echo after");
    assert!(stdout(&out).contains("ok"), "stdout={}", stdout(&out));
    assert!(stdout(&out).contains("after"), "stdout={}", stdout(&out));
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
}

/// Redirecting the inner's stdout does not move the report.
#[test]
fn inner_redirect_leaves_report_on_stderr() {
    let out = run("time ls /dev/null > /dev/null");
    assert_eq!(stdout(&out), "");
    assert_eq!(fields(&stderr(&out)).len(), 3, "stderr={}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

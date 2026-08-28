#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// `ulimit -n N; ulimit -n` sets and reads back the open-file limit.
#[test]
fn set_and_get_nofile() {
    let (out, err, code) = run("ulimit -n 64; ulimit -n");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim(), "64", "stdout={out}");
}

/// `ulimit -f N; ulimit -f` on the block-counted file size.
#[test]
fn set_and_get_fsize() {
    let (out, err, code) = run("ulimit -f 10; ulimit -f");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim(), "10", "stdout={out}");
}

/// No resource flag: bash's default is file size.
#[test]
fn default_resource_is_fsize() {
    let (out, err, code) = run("ulimit 10; ulimit");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim(), "10", "stdout={out}");
}

/// `-H` scope: lower the hard limit (after lowering the soft, since a hard
/// limit may not go below the current soft), then print it with `-H`.
#[test]
fn hard_scope_sets_and_prints() {
    let (out, err, code) = run("ulimit -Sn 64; ulimit -Hn 64; ulimit -Hn");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim(), "64", "stdout={out}");
}

/// `-Sn` sets the soft limit; a bare `ulimit -n` prints the soft, and
/// `ulimit -Hn` prints the (unchanged) hard, which must be at least the soft.
#[test]
fn soft_scope_and_hard_at_least_soft() {
    let (out, err, code) = run("ulimit -Sn 64; ulimit -n; ulimit -Hn");
    assert_eq!(code, 0, "stderr={err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.first(), Some(&"64"), "stdout={out}");
    let hard = lines.get(1).copied().unwrap();
    if hard != "unlimited" {
        assert!(
            hard.parse::<u64>().unwrap() >= 64,
            "hard must stay at or above the soft, stdout={out}"
        );
    }
}

/// `unlimited` works on unit-bearing resources (no 1024× overflow).
#[test]
fn unlimited_on_unit_resource() {
    let (out, err, code) = run("ulimit -f unlimited; ulimit -f");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.trim(), "unlimited", "stdout={out}");
}

/// `-t` is in seconds: the kernel must see 10, not 10240.
#[test]
fn cpu_limit_is_seconds_not_kbytes() {
    let (out, err, code) = run("ulimit -t 10; cat /proc/self/limits");
    assert_eq!(code, 0, "stderr={err}");
    let line = out
        .lines()
        .find(|l| l.starts_with("Max cpu time"))
        .unwrap_or("");
    let fields: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(fields.get(3), Some(&"10"), "cpu line={line:?}");
}

/// `-HS` sets both the soft and the hard limit to the same value.
#[test]
fn both_scopes_set_both_limits() {
    let (out, err, code) = run("ulimit -HSn 64; ulimit -n; ulimit -Hn");
    assert_eq!(code, 0, "stderr={err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.first(), Some(&"64"), "stdout={out}");
    assert_eq!(lines.get(1), Some(&"64"), "stdout={out}");
}

/// `ulimit -a` lists all ten resources with their units and flags.
#[test]
fn list_all_resources() {
    let (out, err, code) = run("ulimit -a");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out.lines().count(), 10, "stdout={out}");
    let expected = [
        "core file size",
        "data seg size",
        "file size",
        "max locked memory",
        "max memory size",
        "open files",
        "stack size",
        "cpu time",
        "max user processes",
        "virtual memory",
    ];
    for name in &expected {
        assert!(out.contains(name), "missing {name:?}, stdout={out}");
    }
    for marker in [
        "(blocks, -c)",
        "(kbytes, -d)",
        "(blocks, -f)",
        "(kbytes, -l)",
        "(kbytes, -m)",
        "(-n)",
        "(kbytes, -s)",
        "(seconds, -t)",
        "(-u)",
        "(kbytes, -v)",
    ] {
        assert!(out.contains(marker), "missing {marker:?}, stdout={out}");
    }
}

/// An unknown option is a clean error (exit 1), nothing on stdout.
#[test]
fn invalid_option_errors() {
    let (out, err, code) = run("ulimit -z");
    assert_eq!(code, 1);
    assert!(err.contains("invalid option"), "stderr={err}");
    assert!(out.is_empty(), "stdout={out}");
}

/// A non-numeric value is a clean error (exit 1), nothing on stdout.
#[test]
fn bad_value_errors() {
    let (out, err, code) = run("ulimit -n abc");
    assert_eq!(code, 1);
    assert!(err.contains("not a limit value"), "stderr={err}");
    assert!(out.is_empty(), "stdout={out}");
}

/// Raising a limit above the hard limit without privilege fails with EPERM.
/// The hard limit is lowered first so the outcome does not depend on the
/// host's default caps.
#[test]
fn raising_above_hard_limit_requires_privilege() {
    let (out, err, code) = run("ulimit -Sn 50; ulimit -Hn 100; ulimit -n 200");
    assert_eq!(code, 1);
    assert!(err.contains("failed to set"), "stderr={err}");
    assert!(out.is_empty(), "stdout={out}");
}

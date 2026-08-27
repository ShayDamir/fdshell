#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// A command substitution with an unbounded producer (`$(yes)`) must hit the
/// capture limit: the child is killed and a clean, actionable error is
/// reported instead of an out-of-memory kill or an infinite hang.
#[test]
fn cmd_subst_unbounded_output_is_capped() {
    let output = Command::new(BIN)
        .args(["-c", "echo $(yes)"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("capture limit"),
        "expected the capture-limit error, exit={:?} stderr={}",
        output.status.code(),
        stderr
    );
    assert!(
        !output.status.success(),
        "oversized command substitution should fail the command"
    );
}

/// A `)` inside double quotes in `$(…)` is data, not the substitution's
/// terminator. Without quote tracking the substitution ends at the first `)`
/// and the rest of the line is re-parsed as shell syntax (injection class).
#[test]
fn cmd_subst_paren_in_quotes_is_data() {
    let output = Command::new(BIN)
        .args(["-c", "x=$(echo \"a)b\"); echo got:$x"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        output.status.success(),
        "paren in quotes must not break the parse, stderr={stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "got:a)b",
        "quoted ) must stay inside the substitution, stdout={stdout:?}"
    );
}

/// Nested `$(…)` inside double quotes: the inner `)` is data for the outer
/// substitution and a real terminator for the inner one.
#[test]
fn cmd_subst_nested_with_quotes() {
    let output = Command::new(BIN)
        .args(["-c", "x=$(echo \"$(echo n)e\"); echo got:$x"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        output.status.success(),
        "nested substitution with quotes must succeed, stderr={stderr}"
    );
    assert_eq!(stdout.trim(), "got:ne");
}

/// A command substitution whose output is under the limit must still work and
/// strip trailing newlines (command substitution semantics).
#[test]
fn cmd_subst_small_output_strips_newlines() {
    let output = Command::new(BIN)
        .args(["-c", "echo [$(echo hello)]"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        "[hello]",
        "trailing newline must be stripped, stdout={stdout:?}"
    );
}

/// `set --stdout-capture-limit` lowers the cap: output above the new cap
/// fails with the same capture-limit error as the default cap.
#[test]
fn capture_limit_lowered_blocks_output() {
    let output = Command::new(BIN)
        .args([
            "-c",
            "set --stdout-capture-limit 5; echo [$(printf \"abcdefgh\")]",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("capture limit"),
        "8 bytes over a 5-byte cap must hit the limit, stderr={stderr}"
    );
    assert!(!output.status.success());
}

/// Output exactly at the new cap is allowed.
#[test]
fn capture_limit_at_cap_allows_output() {
    let output = Command::new(BIN)
        .args([
            "-c",
            "set --stdout-capture-limit 8; x=$(printf \"abcdefgh\"); echo [$x]",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(output.status.success(), "stderr={stderr}");
    assert_eq!(stdout.trim(), "[abcdefgh]");
}

/// A zero cap blocks any non-empty output but still allows empty output.
#[test]
fn capture_limit_zero_allows_empty_only() {
    let output = Command::new(BIN)
        .args(["-c", "set --stdout-capture-limit 0; x=$(true); echo ok"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let stdout = str::from_utf8(&output.stdout).unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(output.status.success(), "stderr={stderr}");
    assert_eq!(stdout.trim(), "ok");
}

/// Missing or non-numeric values are clean errors, not an external `set`
/// fall-through.
#[test]
fn capture_limit_bad_value_is_an_error() {
    for script in [
        "set --stdout-capture-limit",
        "set --stdout-capture-limit abc",
    ] {
        let output = Command::new(BIN)
            .args(["-c", script])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap();

        let stderr = str::from_utf8(&output.stderr).unwrap();
        assert!(
            stderr.contains("not a byte count"),
            "{script:?} must report the bad value, stderr={stderr}"
        );
        assert!(!output.status.success());
    }
}

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

#[test]
fn bare_set_lists_positionals_and_variables() {
    let (out, err, code) = run("FOO=bar; set -- one two; set");
    assert_eq!(code, 0, "stderr={err:?}");
    for line in ["one", "two", "FOO=bar", "_=two"] {
        assert!(
            out.lines().any(|l| l == line),
            "missing {line:?} in {out:?}"
        );
    }
    // Positionals are listed raw, without `NAME=`.
    assert!(!out.contains("one="));
}

#[test]
fn set_lists_exported_variables() {
    let (out, err, code) = run("export P=1; set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == "P=1"), "stdout={out:?}");
}

#[test]
fn set_dash_f_lists_fd_variables() {
    let (out, err, code) = run("set -F");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == "%CWD"), "stdout={out:?}");
}

#[test]
fn set_dash_f_is_empty_without_fd_vars() {
    let (out, err, code) = run("set -F");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().all(|l| l.starts_with('%')), "stdout={out:?}");
}

#[test]
fn set_lists_variables_in_sorted_order() {
    let (out, err, code) = run("ZED=1; ALPHA=2; export MID=9; set");
    assert_eq!(code, 0, "stderr={err:?}");
    // Bare `set` lists `$1..` only, and `-c` with no extra args has none
    // (bash 5.3.9 lists only variables and functions, no positionals at all —
    // accepted divergence).
    // IFS's default value is " \t\n", so its line is followed by a blank one.
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        ["ALPHA=2", "IFS= \t", "", "MID=9", "ZED=1", "_=MID=9"]
    );
}

#[test]
fn set_lists_ifs_exactly_once() {
    let (out, err, code) = run("set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.lines().collect::<Vec<_>>(), ["IFS= \t", ""]);
    // IFS already assigned: still listed exactly once.
    let (out, err, code) = run("IFS=z; set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out.lines().collect::<Vec<_>>(), ["IFS=z", "_="]);
}

#[test]
fn set_lists_exported_variable_not_twice() {
    let (out, err, code) = run("Q=3; export Q; set");
    assert_eq!(code, 0, "stderr={err:?}");
    let qs: Vec<&str> = out.lines().filter(|l| *l == "Q=3").collect();
    assert_eq!(qs, ["Q=3"], "stdout={out:?}");
}

/// Task #281: `set --` substitutes its operand words with their own quote mask,
/// so a quoted character in a tilde-prefix suppresses tilde expansion (POSIX
/// #2.6.1) — bash's `set -- ~"/*"` makes `$1` the text `~/*`.
#[test]
fn positional_with_quoted_tilde_prefix_stays_literal() {
    let (out, err, code) = run(r#"set -- ~"/*"; printf "[%s]\n" "$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[~/*]\n", "bash gets [~/*]");
    let (out, err, code) = run(r#"set -- ~"/x"; printf "[%s]\n" "$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[~/x]\n", "bash gets [~/x]");
    // Contrast: the quote after the `/` leaves the prefix unquoted, so bash
    // makes `$1` `<HOME>/x` — read in-shell, never a hardcoded home path.
    let (home, _err, code) = run(r#"printf "%s" "$HOME""#);
    assert_eq!(code, 0);
    let (out, err, code) = run(r#"set -- ~/"x"; printf "[%s]\n" "$1""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("[{home}/x]\n"), "bash gets [{home}/x]");
}

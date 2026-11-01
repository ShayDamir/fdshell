#![allow(clippy::unwrap_used)]

//! The startup `IFS` has **one owner**: the shell's own stores (task #388).
//! POSIX XCU 2.5.1 — and bash 5.3.9 — have the shell set `IFS` to its default
//! value at startup **even when the parent exported it**, so an inherited row
//! contributes only its export attribute. Every row below was measured in both
//! shells on this branch before being pinned; `—` means no `IFS` row in the
//! parent environment, spelled here as a `"\0"` value. The default split set is
//! the three bytes space, tab, newline.
//!
//! ```text
//!                                                        bash        fdshell
//! $IFS, inherited IFS=,                                  [ \t\n]     [ \t\n]
//! $IFS, —                                                [ \t\n]     [ \t\n]
//! env | grep "^IFS=", inherited IFS=,                    IFS= \t     IFS= \t
//! grep -c "^IFS=", inherited IFS=, / —                   1 / 0       1 / 0
//! IFS=, ; set -- a,b; for w in $1                        [a,b]       [a,b]
//! IFS=, ; IFS=" "; set -- "a b"; for w in $1             [a] [b]     [a] [b]
//! IFS=: builtin true; ${IFS} (prefix restored)           [ \t\n]     [ \t\n]
//! f(){ local IFS=:; ${IFS}; }; f; ${IFS}                 [:] [ \t\n] [:] [ \t\n]
//! set -u; ${IFS}, — (rc)                                 [ \t\n] (0) [ \t\n] (0)
//! unset IFS; ${IFS}, inherited IFS=, (then grep -c)      [] (0) 0    [] (0) 0
//! ```
//!
//! The parenthesised number is the status of the row's own command; a `grep`
//! that matched nothing exits 1 and both shells report it.
//!
//! The last row is task #290's pin kept green: the seed is a **store entry**, so
//! its removal is what "unbound" means, and `unset_var` is unchanged.

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");
const HELPER: &str = env!("EXEC_OK_PATH");

/// The default split set, spelled out once so every expectation states bytes.
const DEFAULT: &str = " \t\n";

/// The helper dir first, then the ambient `PATH` (Nixos has no `/usr/bin`), so
/// the child-side `env`/`grep` of the environment rows resolve (the
/// `tests/cd_expand.rs` shape).
fn test_path() -> String {
    let path_dir = HELPER
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("/usr/bin");
    let ambient = std::env::var("PATH").unwrap_or_default();
    format!("{path_dir}:{ambient}")
}

/// `fdshell -c` with the child's environment spelled out pair by pair (the shape
/// of `tests/unset.rs::run_env`): a `"\0"` value **removes** the key. Every test
/// here names `IFS` explicitly — the fix's rows are exactly the rows an ambient
/// `IFS` moves, so none of them may rely on the harness's.
fn run_env(env: &[(&str, &str)], script: &str) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(["-c", script])
        .env("PATH", test_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        if *value == "\0" {
            cmd.env_remove(key);
        } else {
            cmd.env(key, value);
        }
    }
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

const INHERITED: &[(&str, &str)] = &[("IFS", ",")];
const NO_ROW: &[(&str, &str)] = &[("IFS", "\0")];

/// `$IFS` answers the default in both starts — the inherited row's comma does
/// not reach it, because the shell set the value at startup.
#[test]
fn startup_sets_ifs_to_the_default_whatever_the_parent_exported() {
    let (out, err, code) = run_env(INHERITED, r#"builtin echo "[${IFS}]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("[{DEFAULT}]\n"), "bash measures the same");

    let (out, err, code) = run_env(NO_ROW, r#"builtin echo "[${IFS}]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("[{DEFAULT}]\n"), "bash measures the same");
}

/// The inherited row keeps its place in the child environment — that is the
/// export attribute the parent had — but carries the default value. `env` writes
/// the value's newline as the line break, so the matched line is `IFS= ` + tab.
#[test]
fn the_inherited_row_is_exported_with_the_default_value() {
    let (out, err, code) = run_env(INHERITED, "env | grep '^IFS='");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "IFS= \t\n", "bash measures the same");
}

/// The export attribute itself: inherited → one line, no row → none (bash). Only
/// the value of the inherited row moves; its existence does not.
#[test]
fn the_export_attribute_is_inherited_and_stays_alone() {
    let (out, err, code) = run_env(INHERITED, "env | grep -c '^IFS='");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "1\n", "bash measures the same");

    let (out, _err, code) = run_env(NO_ROW, "env | grep -c '^IFS='");
    assert_eq!(code, 1, "grep's no-match status, which bash reports too");
    assert_eq!(out, "0\n", "no row means no export, in bash too");
}

/// No splitting behaviour moves (task #290's rule): the inherited comma is not a
/// separator, and an in-shell assignment is — both in one shell, in both shells.
#[test]
fn the_split_set_still_ignores_the_inherited_value() {
    let (out, err, code) = run_env(
        INHERITED,
        r#"set -- a,b; for w in $1; do builtin echo "[$w]"; done"#,
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a,b]\n", "the comma splits in bash, not here");

    let (out, err, code) = run_env(
        INHERITED,
        r#"IFS=" "; set -- "a b"; for w in $1; do builtin echo "[$w]"; done"#,
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[a]\n[b]\n", "the assigned space splits both shells");
}

/// A scoped prefix is undone on restore, so `$IFS` is the startup value again
/// (the entry half of the frame; the split set is the field half — LESSONS).
#[test]
fn a_scoped_prefix_is_restored_and_ifs_reads_the_default_again() {
    let (out, err, code) = run_env(INHERITED, r#"IFS=: builtin true; builtin echo "[${IFS}]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("[{DEFAULT}]\n"), "bash measures the same");
}

/// The `local` unwind puts the seed back: the name reads `:` inside the call and
/// the default after it (bash).
#[test]
fn the_local_frame_unwinds_ifs_back_to_the_default() {
    let script = r#"f(){ local IFS=:; builtin echo "[${IFS}]"; }; f; builtin echo "[${IFS}]""#;
    let (out, err, code) = run_env(NO_ROW, script);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("[:]\n[{DEFAULT}]\n"), "bash measures the same");
}

/// With no inherited row, the startup seed is what makes the name **bound** under
/// `set -u` — bash reports the default and exits 0 (no row, so nothing is
/// exported to the child of an `env` here; only the expansion is in question).
#[test]
fn nounset_reports_the_default_at_startup() {
    let (out, err, code) = run_env(NO_ROW, r#"set -u; builtin echo "[${IFS}]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out,
        format!("[{DEFAULT}]\n"),
        "bash measures the same, rc 0"
    );
}

/// `unset IFS` keeps task #290's meaning: the split set is the default, the
/// **name** is unbound (reads empty), and the child environment loses the row —
/// in both shells, with the comma inherited.
#[test]
fn unset_ifs_still_unbinds_the_name() {
    let (out, err, code) = run_env(INHERITED, r#"unset IFS; builtin echo "[${IFS}]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out, "[]\n",
        "the name is gone, the split set is the default"
    );

    let (out, _err, code) = run_env(INHERITED, r#"unset IFS; env | grep -c '^IFS='"#);
    assert_eq!(code, 1, "grep found nothing, and bash exits 1 on that too");
    assert_eq!(out, "0\n", "unset drops the export attribute too (bash)");
}

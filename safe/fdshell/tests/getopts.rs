#![allow(clippy::unwrap_used)]

//! `getopts` resolves as an **in-shell intercept**, and the `OPTIND` state it
//! needs is seeded at startup as a visible variable (task #589). Every row below
//! was measured in both shells on this machine on 2026-11-16 before being pinned;
//! the harness removes `OPTIND` and `OPTARG` from the shell's own environment, so a
//! row in a child environment can only come from fdshell's stores, and the child
//! rows use the `env_print` helper (no external `env`/`grep`).
//!
//! ```text
//!                                                        bash        fdshell
//! builtin echo "[$OPTIND]"                               [1]        [1]
//! the OPTIND row of `set`                                OPTIND=1    OPTIND=1
//! OPTIND=7; OPTARG=z; env_print (rows)                   0           0
//! OPTIND=7; export OPTIND; env_print (rows)              1           1
//! OPTARG=z; builtin echo "[$OPTARG]"                     [z]         [z]
//! OPTARG=z; unset OPTARG; builtin echo "[$OPTARG]"       []          []
//! set -- -a; getopts a o  (stderr)                       (empty)     (empty)
//! f(){ local OPTIND=3; }; f; builtin echo "[$OPTIND]"    [1]        [1]
//! f(){ OPTIND=3; }; f; builtin echo "[$OPTIND]"          [3]        [3]
//! OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]"  [1]        [1]
//! unset OPTIND; OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]"  []  []
//! ```
//!
//! The scan itself is task #590, so the last row pins only that `getopts` is
//! **claimed** — the forked launch path's `failed to resolve command path` is gone.
//! The status is not pinned (bash's no-operand call exits 1), and neither is bash's
//! integer coercion of an unparseable `OPTIND` (bash's `$OPTIND` answers `0`,
//! fdshell's the assigned bytes) — both are #590/#591 decisions.

use std::process::{Command, Stdio};
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// The helper that prints **its own** environment (built by `build.rs`).
const ENV_PRINT: &str = env!("ENV_PRINT_PATH");

/// The helper dir first, then the ambient `PATH` (Nixos has no `/usr/bin`), so the
/// script's `env_print` call resolves (the `tests/cd_expand.rs` shape).
fn test_path() -> String {
    let path_dir = ENV_PRINT
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("/usr/bin");
    let ambient = std::env::var("PATH").unwrap_or_default();
    format!("{path_dir}:{ambient}")
}

/// `fdshell -c`, with `OPTIND` and `OPTARG` **removed** from the shell's
/// environment: a row in a child environment is then fdshell's own doing, never the
/// harness's (the `tests/ifs_startup.rs::run_env` shape).
fn run(script: &str) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(["-c", script])
        .env("PATH", test_path())
        .env_remove("OPTIND")
        .env_remove("OPTARG")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// The seed is a real variable: `$OPTIND` answers it and `set` lists it (bash
/// measures `OPTIND=1`).
#[test]
fn the_startup_optind_is_visible_in_shell() {
    let (out, err, code) = run(r#"builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[1]\n", "bash measures the same");

    let (out, err, code) = run("set");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(out.lines().any(|l| l == "OPTIND=1"), "stdout={out:?}");
}

/// An ordinary assignment does **not** export either name — bash measures zero rows
/// — and an explicit `export` is the only exporter (bash measures one row there).
#[test]
fn optind_and_optarg_stay_out_of_the_child_environment() {
    let (out, err, code) = run("OPTIND=7; OPTARG=z; env_print");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        !out.lines().any(|l| l.starts_with("OPTIND=")),
        "stdout={out:?}"
    );
    assert!(
        !out.lines().any(|l| l.starts_with("OPTARG=")),
        "stdout={out:?}"
    );

    let (out, err, code) = run("OPTIND=7; export OPTIND; env_print");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(
        out.lines().any(|l| l == "OPTIND=7"),
        "the ordinary export path exports, in bash too"
    );
}

/// `OPTARG` needs **no field**: it is a plain store variable, so the ordinary
/// assignment and `unset` paths are the whole story here (the scan sets it in #590).
#[test]
fn optarg_is_an_ordinary_variable_on_the_assignment_path() {
    let (out, err, code) = run(r#"OPTARG=z; builtin echo "[$OPTARG]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[z]\n", "bash measures the same");

    let (out, err, code) = run(r#"OPTARG=z; unset OPTARG; builtin echo "[$OPTARG]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n", "bash measures the same");
}

/// `getopts` is **claimed** as an intercept, so the forked launch path no longer
/// fails it with `failed to resolve command path` (POSIX gaps #6.9). Nothing on
/// stderr is the bash-measured row; the status is left to #590.
#[test]
fn getopts_no_longer_fails_to_resolve_its_path() {
    let (out, err, _code) = run("set -- -a; getopts a o");
    assert_eq!(err, "", "no diagnostic; stdout={out:?}");
}

/// A `local OPTIND=3` is undone when the call returns — the frame's counter
/// mirror (`old_optind`) rides the same restore as the store entry. The plain
/// assignment control has no frame entry and **leaks** (task #593, bash rows
/// measured on this machine).
#[test]
fn local_optind_is_restored_when_the_call_returns() {
    let (out, err, code) = run(r#"f(){ local OPTIND=3; }; f; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[1]\n", "bash measures the same");

    let (out, err, code) = run(r#"f(){ OPTIND=3; }; f; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(
        out, "[3]\n",
        "no frame entry, so the assignment leaks, in bash too"
    );
}

/// The scoped prefix is undone after the command (the visible half of
/// `EnvSave::restore`; the plain-assignment control of #589 stays as pinned).
#[test]
fn a_scoped_optind_prefix_is_undone_after_the_command() {
    let (out, err, code) = run(r#"OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[1]\n", "bash measures the same");
}

/// Unbound before the prefix, unbound after it: the name has no saved entry to
/// restore, and only the saved counter brings the state back (bash answers `[]`,
/// task #593).
#[test]
fn an_unbound_optind_stays_unbound_across_a_scoped_prefix() {
    let (out, err, code) =
        run(r#"unset OPTIND; OPTIND=3 env_print > /dev/null; builtin echo "[$OPTIND]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n", "bash measures the same");
}

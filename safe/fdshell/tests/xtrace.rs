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
fn set_dash_x_traces_commands_on_stderr() {
    let (out, err, code) = run("set -x; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    assert!(err.contains("+ set -x"), "stderr={err:?}");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
}

#[test]
fn set_plus_x_stops_tracing() {
    let (_out, err, code) = run("set -x; set +x; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ set -x"), "stderr={err:?}");
    // bash traces the disabling command itself.
    assert!(err.contains("+ set +x"), "stderr={err:?}");
    assert!(!err.contains("+ echo hi"), "stderr={err:?}");
}

#[test]
fn xtrace_shows_expanded_args() {
    let (_out, err, code) = run("X=1; set -x; builtin echo $X");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo 1"), "stderr={err:?}");
}

#[test]
fn xtrace_covers_intercepts() {
    let (_out, err, code) = run("set -x; shift; builtin echo ok");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ shift"), "stderr={err:?}");
}

#[test]
fn xtrace_covers_pipeline_stages() {
    let (_out, err, code) = run("set -x; builtin echo a | builtin echo b");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo a"), "stderr={err:?}");
    assert!(err.contains("+ echo b"), "stderr={err:?}");
}

#[test]
fn xtrace_settable_via_set_dash_o() {
    let (_out, err, code) = run("set -o xtrace; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
    let (_out, err, code) = run("shopt -s xtrace; builtin echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
}

/// `set -x` on a joined list (task #492, POSIX #2.9): one statement, and only the
/// commands that run are traced — the short-circuited side leaves no trace line,
/// exactly as on the same-line spelling. Measured 2026-10-30 against bash 5.3.9
/// and this build.
#[test]
fn xtrace_of_a_joined_cond_list_traces_only_what_runs() {
    for (script, ran, skipped, rc) in [
        ("set -x; false &&\necho b", "false", "echo b", 1),
        ("set -x; true ||\necho b", "true", "echo b", 0),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), ("", rc), "{script}: {err}");
        assert!(err.contains(ran), "{script}: {ran} ran: {err}");
        assert!(
            !err.contains(skipped),
            "{script}: {skipped} was skipped: {err}"
        );
    }
}

/// The same run with the environment's `PS4` set (`Some`) or absent (`None`) — the
/// two halves of the variable lookup: an inherited `PS4=X fdshell -c …` answers
/// through the `environ` snapshot, an absent one through the read-time default
/// (task #621; every row below measured 2026-11-20 against bash 5.3.9 and this
/// build, and the rows the shells disagree on say so).
fn run_ps4(ps4: Option<&str>, script: &str) -> (String, String, i32) {
    let mut command = Command::new(BIN);
    command
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match ps4 {
        Some(value) => {
            command.env("PS4", value);
        }
        None => {
            command.env_remove("PS4");
        }
    }
    let output = command.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn ps4_from_the_environment_is_the_prefix() {
    // bash: `PS4='* ' bash -c 'set -x; echo hi'` → `* echo hi`.
    let (out, err, code) = run_ps4(Some("* "), "set -x; echo hi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    assert!(err.contains("* echo hi"), "stderr={err:?}");
}

#[test]
fn ps4_assigned_as_a_command_is_the_prefix() {
    // bash: `PS4='[x] '; set -x; echo hi` → `[x] echo hi`. The single quotes keep
    // the stored bytes raw in both shells, so the row pins the same input.
    let (out, err, code) = run_ps4(None, "PS4='[x] '; set -x; echo hi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    assert!(err.contains("[x] echo hi"), "stderr={err:?}");
}

#[test]
fn ps4_escapes_decode_before_the_prefix_is_drawn() {
    // bash: `PS4='A\aeB\n' …` → `A\x07B` and a real newline, so the `\n` splits the
    // trace line in both shells.
    let (_out, err, code) = run_ps4(Some("A\\aeB\\n"), "set -x; echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("A\u{7}eB\necho hi"), "stderr={err:?}");
}

#[test]
fn ps4_is_expanded_at_each_use() {
    // bash: `v=V; PS4='[$v] '; set -x; v=W; echo hi` → `[V] v=W` then `[W] echo hi`
    // — the value is re-expanded per line, not stored expanded. bash's `[V] v=W` row
    // is the known "a plain assignment is not traced here" divergence, so the pin is
    // the last trace line.
    let (_out, err, code) = run_ps4(None, "v=V; PS4='[$v] '; set -x; v=W; echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("[W] echo hi"), "stderr={err:?}");
}

#[test]
fn a_command_substitution_in_ps4_runs_per_line() {
    // bash: `PS4='A\ae$(echo S) ' …` → `A\u{7}eS echo hi`. fdshell draws that line
    // with the same bytes; the difference is the road — the `$( … )` child inherits
    // xtrace and traces its own command, so a `PS4` naming a command substitution
    // prints a line per nested level here, where bash's prompt expansion prints
    // nothing (README, task #621). The pin is the line both shells agree on.
    let (_out, err, code) = run_ps4(Some("A\\ae$(echo S) "), "set -x; echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("A\u{7}eS echo hi"), "stderr={err:?}");
}

#[test]
fn ps4_follows_the_promptvars_shopt() {
    // Measured: the shopt gates `PS4` as it gates `PS1`/`PS2` — off, the stored
    // bytes are drawn as they stand; on (the default) they expand. fdshell agrees
    // on both rows.
    let (_out, err, code) = run_ps4(
        None,
        "v=V; shopt -u promptvars; PS4='[$v] '; set -x; echo hi",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("[$v] echo hi"), "stderr={err:?}");
    let (_out, err, code) = run_ps4(
        None,
        "v=V; shopt -s promptvars; PS4='[$v] '; set -x; echo hi",
    );
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("[V] echo hi"), "stderr={err:?}");
}

#[test]
fn a_set_empty_ps4_draws_no_prefix() {
    // bash: `PS4=; set -x; echo hi` → `echo hi`, no prefix. Both shells print the
    // `set -x` line fdshell has always printed for it, so both trace lines here
    // start at column 0.
    let (out, err, code) = run_ps4(None, "PS4=; set -x; echo hi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    assert_eq!(err, "set -x\necho hi\n");
}

#[test]
fn bash_command_in_ps4_expands_empty_here() {
    // bash: `PS4='[${BASH_COMMAND}] ' …` → `[echo hi] echo hi`, because bash
    // maintains its `BASH_*` variables per trace line. Here the three are ordinary
    // variables and an unset one expands empty — an assigned one works in both
    // shells (README, task #621).
    let (_out, err, code) = run_ps4(None, "PS4='[${BASH_COMMAND}] '; set -x; echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert!(err.contains("[] echo hi"), "stderr={err:?}");
}

#[test]
fn an_unset_ps4_is_no_variable_here() {
    // bash *seeds* `PS4` as a real variable: `echo [$PS4]` answers `[+ ]` there and
    // `[]` here, and after `PS4=Z; unset PS4` bash traces with no prefix while the
    // read-time fallback draws `+ ` here (decision D-621-1; the seed row is the
    // alternative the plan left open).
    let (out, err, code) = run_ps4(None, "unset PS4; echo [$PS4]");
    assert_eq!((out.as_str(), code), ("[]\n", 0), "stderr={err:?}");
    let (out, err, code) = run_ps4(None, "PS4=Z; unset PS4; set -x; echo hi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
}

#[test]
fn a_failing_expansion_in_ps4_is_reported_and_the_shell_continues() {
    // bash reports `nope: unbound variable` for that script and exits. Here the
    // prompt layer's existing rule applies and is inherited for free: report, paste
    // the run unexpanded, keep going — the nounset-at-assignment family, not a new
    // `PS4` rule.
    let (out, err, code) = run_ps4(None, "set -u; PS4='[$nope] '; set -x; echo hi");
    assert_eq!((out.as_str(), code), ("hi\n", 0), "stderr={err:?}");
    assert!(err.contains("nope: unbound variable"), "stderr={err:?}");
    assert!(err.contains("[$nope] echo hi"), "stderr={err:?}");
}

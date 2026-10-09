#![allow(clippy::unwrap_used)]

use std::process::{Command, Stdio};
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

static COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// Run with the child's cwd set to `cwd` (for the noglob scratch dir).
fn run_in(cwd: &str, script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .current_dir(cwd)
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

fn scratch() -> String {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-setopts-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a1", "a2"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    dir.to_str().unwrap().to_string()
}

// --- -e / +e (errexit) ---

#[test]
fn errexit_stops_the_shell_on_failing_command() {
    let (out, _err, code) = run("set -e; false; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
}

#[test]
fn errexit_off_again_runs_the_rest() {
    let (out, _err, code) = run("set -e; set +e; false; echo after");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "after\n");
}

#[test]
fn errexit_and_list_failure_is_exempt() {
    // `false` is not the command following the final `&&`: exempt (bash).
    let (out, _err, code) = run("set -e; false && echo x; echo after");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "after\n");
}

#[test]
fn errexit_final_and_command_fires() {
    let (out, _err, code) = run("set -e; true && false; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
}

#[test]
fn errexit_if_condition_is_exempt() {
    let (out, _err, code) = run("set -e; if false; then echo y; fi; echo after");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "after\n");
}

#[test]
fn errexit_while_condition_is_exempt() {
    let (out, _err, code) = run("set -e; while false; do echo y; done; echo after");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "after\n");
}

#[test]
fn errexit_loop_body_fires() {
    let (out, _err, code) = run("set -e; for x in a b; do false; done; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
}

#[test]
fn errexit_uses_pipeline_last_status() {
    let (out, _err, code) = run("set -e; true | false; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
    let (out, _err, code) = run("set -e; false | true; echo after");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "after\n");
}

#[test]
fn errexit_function_body_fires() {
    let (out, _err, code) = run("set -e; f() { false; }; f; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
}

// --- -u / +u (nounset) ---

#[test]
fn nounset_unbound_var_fails() {
    let (_out, err, code) = run("set -u; echo $x");
    assert_eq!(code, 1);
    assert!(err.contains("x: unbound variable"), "stderr={err:?}");
}

#[test]
fn nounset_bound_var_is_fine() {
    let (out, _err, code) = run("set -u; X=1; echo $X");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "1\n");
}

#[test]
fn nounset_off_expands_empty() {
    // `set +u` restores the POSIX rule: an unset parameter expands to empty.
    let (out, _err, code) = run("set -u; set +u; echo $x");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "\n");
}

#[test]
fn nounset_out_of_range_positional_fails() {
    let (_out, err, code) = run("set -u; echo $1");
    assert_eq!(code, 1);
    assert!(err.contains("1: unbound variable"), "stderr={err:?}");
}

// --- -f / +f (noglob) ---

#[test]
fn noglob_keeps_star_literal() {
    let (out, _err, code) = run("set -f; echo *");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "*\n");
}

#[test]
fn noglob_off_expands_again() {
    let dir = scratch();
    let (out, _err, code) = run_in(&dir, "set -f; set +f; echo a*");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "a1 a2\n");
}

#[test]
fn noglob_on_keeps_matching_pattern_literal() {
    let dir = scratch();
    let (out, _err, code) = run_in(&dir, "set -f; echo a*");
    assert_eq!(code, 0, "out={out:?}");
    assert_eq!(out, "a*\n");
}

// --- -v / +v (verbose) ---

#[test]
fn verbose_echoes_statements_to_stderr() {
    let (out, err, code) = run("set -v; echo hi");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\n");
    assert!(err.contains("echo hi"), "stderr={err:?}");
}

#[test]
fn verbose_off_again_stops_echoing() {
    let (_out, err, code) = run("set -v; builtin echo hi; set +v; builtin echo bye");
    assert_eq!(code, 0);
    assert!(err.contains("builtin echo hi"), "stderr={err:?}");
    assert!(!err.contains("builtin echo bye"), "stderr={err:?}");
}

// --- `$-` and free integration ---

#[test]
fn dash_shows_active_short_flags() {
    let (out, _err, code) = run("echo $-");
    assert_eq!(code, 0);
    assert_eq!(out, "\n");
    let (out, _err, code) = run("set -e; echo $-");
    assert_eq!(code, 0);
    assert_eq!(out, "e\n");
    let (out, _err, code) = run("set -u; set -f; set -v; echo $-");
    assert_eq!(code, 0);
    assert_eq!(out, "ufv\n");
}

#[test]
fn posix_flags_settable_via_set_dash_o() {
    let (out, _err, code) = run("set -o errexit; false; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
    let (_out, err, code) = run("set -o nounset; echo $x");
    assert_eq!(code, 1);
    assert!(err.contains("x: unbound variable"), "stderr={err:?}");
    let (out, _err, code) = run("set -o noglob; echo *");
    assert_eq!(code, 0);
    assert_eq!(out, "*\n");
    let (out, _err, code) = run("set -o verbose; echo hi");
    assert_eq!(code, 0);
    assert_eq!(out, "hi\n");
}

#[test]
fn posix_flags_visible_to_shopt() {
    let (out, _err, code) = run("set -f; shopt -q noglob; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
    let (out, _err, code) = run("shopt -q noglob; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

#[test]
fn xtrace_covers_short_flag_set() {
    let (_out, err, code) = run("set -x; set -e; set +x");
    assert_eq!(code, 0);
    assert!(err.contains("+ set -e"), "stderr={err:?}");
}

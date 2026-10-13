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

// --- concatenated short flags (`set -eu`, `set +ef`) ---

#[test]
fn cluster_sets_every_flag_in_the_word() {
    let (out, _err, code) = run("set -eu; echo $-");
    assert_eq!(code, 0, "stderr={_err:?}");
    assert_eq!(out, "eu\n");
}

#[test]
fn cluster_duplicate_flag_is_idempotent() {
    let (out, _err, code) = run("set -ee; echo $-");
    assert_eq!(code, 0, "stderr={_err:?}");
    assert_eq!(out, "e\n");
}

#[test]
fn cluster_nounset_flag_bails_unbound() {
    let (_out, err, code) = run("set -eu; echo $x");
    assert_eq!(code, 1);
    assert!(err.contains("x: unbound variable"), "stderr={err:?}");
}

#[test]
fn cluster_errexit_flag_stops_the_shell() {
    let (out, _err, code) = run("set -eu; false; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
}

#[test]
fn cluster_clears_every_named_flag() {
    let dir = scratch();
    // `-f` alone keeps the pattern literal; `+ef` clears noglob (and errexit).
    let (out, _err, code) = run_in(&dir, "set -f; set +ef; echo a*");
    assert_eq!(code, 0, "stderr={_err:?}");
    assert_eq!(out, "a1 a2\n");
}

#[test]
fn cluster_leaves_the_flags_it_does_not_name() {
    let dir = scratch();
    let (out, _err, code) = run_in(&dir, "set -f; set +e; echo a*");
    assert_eq!(code, 0, "stderr={_err:?}");
    assert_eq!(out, "a*\n", "`f` is untouched by `set +e`");
}

#[test]
fn cluster_verbose_flag_echoes_statements() {
    let (out, err, code) = run("set -uv; builtin echo hi; set +v; builtin echo bye");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\nbye\n");
    assert!(err.contains("builtin echo hi"), "stderr={err:?}");
    assert!(!err.contains("builtin echo bye"), "stderr={err:?}");
}

#[test]
fn cluster_that_turns_on_xtrace_is_not_itself_traced() {
    // bash parity: `set -xf` traces `echo hi`, not the `set` that enabled xtrace.
    let (out, err, code) = run("set -xf; echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    assert!(err.contains("+ echo hi"), "stderr={err:?}");
    assert!(!err.contains("+ set -xf"), "stderr={err:?}");
}

#[test]
fn cluster_is_traced_when_xtrace_is_already_on() {
    let (out, err, code) = run("set -x; set -eu; echo hi");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hi\n");
    assert!(err.contains("+ set -eu"), "stderr={err:?}");
}

#[test]
fn cluster_unknown_flag_is_a_builtin_error() {
    let (_out, err, code) = run("set -ez");
    assert_eq!(code, 1);
    assert!(err.contains("set") && err.contains("-ez"), "stderr={err:?}");

    // fdshell stops the script at the failing `set`; bash reports status 2 and
    // runs the rest (accepted divergence, README).
    let (out, _err, code) = run("set -ez; echo after");
    assert_eq!(code, 1);
    assert!(!out.contains("after"), "stdout={out:?}");
}

#[test]
fn cluster_mid_cluster_sign_is_an_unknown_flag() {
    let (_out, err, code) = run("set -e+u; echo after");
    assert_eq!(code, 1);
    assert!(err.contains("-e+u"), "stderr={err:?}");
}

#[test]
fn cluster_names_the_whole_word_not_the_offending_byte() {
    // bash names the first unknown byte (`set: -c: invalid option`); fdshell
    // names the word (accepted divergence, README).
    let (_out, err, code) = run("set -abc");
    assert_eq!(code, 1);
    assert!(err.contains("-abc"), "stderr={err:?}");
}

#[test]
fn cluster_error_reports_before_the_builtin_keyword_check() {
    let (_out, err, code) = run("builtin set -e");
    assert_eq!(code, 1);
    assert!(err.contains("prefix is not supported"), "stderr={err:?}");
    let (_out, err, code) = run("builtin set -z");
    assert_eq!(code, 1);
    assert!(err.contains("-z"), "stderr={err:?}");
}

/// Only the first word is a cluster (`set -eu -f` applies `-eu`); multi-word
/// flag lists are task #249.
#[test]
fn cluster_only_the_first_word_is_a_flag_list() {
    let (out, _err, code) = run("set -eu -f; echo $-");
    assert_eq!(code, 0, "stderr={_err:?}");
    assert_eq!(out, "eu\n");
}

/// `set -`/`set +` are not clusters (task #249): they keep falling through.
#[test]
fn lone_sign_still_falls_through_to_external_lookup() {
    let (out, err, code) = run("set -; echo after");
    assert!(out.contains("after"), "stdout={out:?}");
    assert!(err.contains("resolve command path"), "stderr={err:?}");
    assert_eq!(code, 0, "stdout={out:?}");
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

/// `set -u` bails an unbound variable in a `for … in` list too (task #131
/// runs the argument pipeline there): rc 1, `unbound variable`, no iteration.
/// bash's `-c` mode exits 127 (LESSONS.md accepts the rc asymmetry).
#[test]
fn for_list_nounset_bails() {
    let (out, err, code) = run("set -u; for w in $undef; do echo hit; done; echo after");
    assert_eq!(code, 1, "stderr={err}");
    assert!(err.contains("undef: unbound variable"), "stderr={err:?}");
    assert_eq!(out, "");
}

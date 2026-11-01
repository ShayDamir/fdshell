#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    run_env(script, &[])
}

/// The same run with extra environment entries — rows whose script lands on
/// `$HOME` need one (the hermetic test sandbox has no home to land on).
fn run_env(script: &str, env: &[(&str, &std::path::Path)]) -> (String, String, i32) {
    let mut cmd = Command::new(BIN);
    cmd.args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let output = cmd.output().unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn last_arg_of_previous_command() {
    let (out, err, code) = run("true a b; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "b\n");
}

#[test]
fn last_arg_is_expanded() {
    let (out, err, code) = run("x=hello; true $x; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "hello\n");
}

#[test]
fn no_args_falls_back_to_command_name() {
    let (out, err, code) = run("true; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "true\n");
}

#[test]
fn quoted_last_arg_stays_one_word() {
    let (out, err, code) = run("true \"last quoted\"; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "last quoted\n");
}

#[test]
fn command_substitution_arg_is_expanded() {
    let (out, err, code) = run("true $(builtin echo sub); builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "sub\n");
}

#[test]
fn failed_command_still_sets_last_arg() {
    let (out, _err, _code) = run("nonexistent_cmd_xyz arg; builtin echo $_");
    assert_eq!(out, "arg\n");
}

#[test]
fn pipeline_does_not_update_last_arg() {
    let (out, err, code) =
        run("builtin echo prev >/dev/null; builtin echo a | grep a; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a\n[prev]\n");
}

#[test]
fn background_does_not_update_last_arg() {
    let (out, err, code) = run("builtin echo prev >/dev/null; true a b &>&bg; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[prev]\n");
}

#[test]
fn plain_assignment_clears_last_arg() {
    let (out, err, code) = run("true a; x=1; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn for_loop_inner_commands_update_last_arg() {
    // Like bash, the loop body's last command sets `$_` (no clear at loop end).
    let (out, err, code) = run("for i in 1 2; do true p q; done; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[q]\n");
}

#[test]
fn eval_keeps_its_own_last_arg() {
    let (out, err, code) = run("eval \"builtin echo b c\"; builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "b c\n[builtin echo b c]\n");
}

#[test]
fn set_positional_updates_last_arg_expanded() {
    let (out, err, code) = run("a=zeta; set -- $a omega; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "omega\n");
}

#[test]
fn intercept_without_args_falls_back_to_command_name() {
    let (out, err, code) = run("cd /tmp; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "/tmp\n");
}

#[test]
fn unset_updates_last_arg() {
    let (out, err, code) =
        run("builtin openat2 --flags O_RDONLY /dev/null %>%f; unset %f; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "f\n");
}

#[test]
fn braced_and_split_forms() {
    let (out, err, code) = run("true a b; x=${_}; builtin echo $x");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "b\n");
}

fn run_without_env_underscore(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
        .env_remove("_")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    (
        str::from_utf8(&output.stdout).unwrap().to_string(),
        str::from_utf8(&output.stderr).unwrap().to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn last_arg_empty_when_never_set() {
    // Inherited `_` is removed: an unset parameter expands to empty, so `$_`
    // prints nothing (the same rule as every other parameter).
    let (out, err, code) = run_without_env_underscore("builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn dollar_underscore_longer_name_stays_variable() {
    // `$_y` is the variable `_y`, not the special `$_` followed by `y`. The
    // name is bound so the longest-match probe is observable (an unset name
    // expands to empty and would print the same `x=`).
    let (out, err, code) = run_without_env_underscore("_y=full; builtin echo x=$_y");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x=full\n");
}

#[test]
fn set_option_updates_last_arg() {
    // Only `set --` stores its own value; `set -o` follows the generic path.
    let (out, err, code) = run("true hello; set -o noclobber; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "noclobber\n");
}

#[test]
fn nounset_binds_last_arg_so_underscore_is_never_unbound() {
    // The `$_` arm has no nounset check, and that is unobservable in `-c` mode:
    // `set -u` binds `_` to its own argument in fdshell exactly as in bash, so
    // `$_` is bound and prints `-u` on both.
    let (out, err, code) = run(r#"set -u; builtin echo "[$_]""#);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[-u]\n");
}

#[test]
fn eval_inner_commands_do_not_update_last_arg() {
    let (out, err, code) = run("true hello; eval \"true x y; builtin echo [$_]\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[hello]\n");
}

// The expanding intercepts hand their expanded field list to the `$_` frame
// (task #376). Every expectation below is a bash 5.3.9 measurement, and the
// fdshell answer matches it; the rows are quoted (`"$_"`) because a bare `[x]`
// argument is a glob pattern in both shells.

#[test]
fn eval_reports_its_expanded_word() {
    let (out, err, code) = run("V=hello; eval \"true $V\"; builtin echo \"$_\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "true hello\n");
}

#[test]
fn eval_reports_the_quoted_word_that_expanded_to_nothing() {
    // A fully quoted word that expands to nothing is an (empty) field, so `$_`
    // is empty — the fallback to the command word is only for no operand words.
    let (out, err, code) = run("eval \"$zz\"; builtin echo \"[$_]\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn let_reports_the_expanded_expression() {
    let (out, err, code) = run("V='x=1'; let \"$V\"; builtin echo \"$_\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x=1\n");
}

#[test]
fn local_reports_the_consumed_word() {
    let (out, err, code) = run("f(){ V=hello; local \"x=$V\"; builtin echo \"$_\"; }; f");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x=hello\n");
}

#[test]
fn wait_reports_the_expanded_pid() {
    // The pid is whatever this run produced, so the row asserts that the two
    // halves of one line agree — bash prints the same pair.
    let (out, err, code) = run("sleep 0 & p=$!; wait $p; builtin echo \"$_=$p\"");
    assert_eq!(code, 0, "stderr={err:?}");
    let halves = out.trim().split('=').collect::<Vec<_>>();
    assert_eq!(halves.len(), 2, "stdout={out:?}");
    assert!(
        halves.first().is_some_and(|p| !p.is_empty()),
        "the pid must be printed, stdout={out:?}"
    );
    assert_eq!(halves.first(), halves.get(1), "the pid is the last field");
}

#[test]
fn source_reports_its_expanded_path() {
    let dir = scratch("source");
    let path = dir.join("f.sh");
    std::fs::write(&path, "true\n").unwrap();
    let script = format!("f={}; source $f; builtin echo \"$_\"", path.display());
    let (out, err, code) = run(&script);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, format!("{}\n", path.display()));
}

#[test]
fn zero_field_expansion_falls_back_to_command_name() {
    // 0 fields: `cd` lands on `$HOME` and `$_` is the command word (bash's
    // answer too, measured). `$HOME` is a scratch dir — the hermetic sandbox
    // gives the test process no home to land on.
    let home = scratch("zero-field-home");
    let (out, err, code) = run_env("cd $zz; builtin echo \"$_\"", &[("HOME", &home)]);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "cd\n");
}

#[test]
fn expanded_command_name_is_the_fallback() {
    // Word 0 arrives expanded (task #268's prehash), so the fallback word is the
    // expansion `cd`; the scratch `$HOME` is the directory the bare `cd` lands on.
    let home = scratch("command-name-home");
    let (out, err, code) = run_env("V=cd; $V; builtin echo \"$_\"", &[("HOME", &home)]);
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "cd\n");
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop: pids are recycled,
/// so a dir that outlives its process is a later run's inherited dir with that
/// run's files in it (LESSONS, task #255; sweep task #342). Removal is
/// `.ok()`-tolerant, so teardown never panics.
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-last-arg-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

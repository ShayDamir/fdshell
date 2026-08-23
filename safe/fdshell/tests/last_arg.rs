#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> (String, String, i32) {
    let output = Command::new(BIN)
        .args(["-c", script])
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
    // Inherited `_` is removed: an ordinary unset variable expands to literal
    // text, but `$_` must expand to empty.
    let (out, err, code) = run_without_env_underscore("builtin echo [$_]");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[]\n");
}

#[test]
fn dollar_underscore_longer_name_stays_variable() {
    // `$_y` is the variable `_y`, not the special `$_` followed by `y`.
    let (out, err, code) = run_without_env_underscore("builtin echo x=$_y");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "x=$_y\n");
}

#[test]
fn set_option_updates_last_arg() {
    // Only `set --` stores its own value; `set -o` follows the generic path.
    let (out, err, code) = run("true hello; set -o noclobber; builtin echo $_");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "noclobber\n");
}

#[test]
fn eval_inner_commands_do_not_update_last_arg() {
    let (out, err, code) = run("true hello; eval \"true x y; builtin echo [$_]\"");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "[hello]\n");
}

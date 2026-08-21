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
fn test_file_exists() {
    let (out, _err, code) = run("if test -f /proc/self/exe; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn bracket_file_is_dir() {
    let (out, _err, code) = run("if [ -d /tmp ]; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn bracket_file_is_not_dir() {
    let (out, _err, code) = run("if [ -f /tmp ]; then printf n; else printf y; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_missing_file_is_false() {
    let (out, _err, code) =
        run("if test -e /nonexistent-fdshell-test; then printf n; else printf y; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_string_equality_with_variables() {
    let (out, _err, code) = run("X=abc; if test \"$X\" = abc; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_string_empty_with_variables() {
    let (out, _err, code) =
        run("X=; if test -z \"$X\"; then printf a; fi; X=hi; if test -n \"$X\"; then printf b; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "ab");
}

#[test]
fn set_dashdash_replaces_positional() {
    let (out, _err, code) = run(r#"X=old; set -- $X two; printf "$0 $1""#);
    assert_eq!(code, 0);
    assert_eq!(out, "old two");
}

#[test]
fn set_dashdash_clears_positional() {
    let (out, _err, code) = run(r#"set -- a b; set --; printf "z$@""#);
    assert_eq!(code, 0);
    assert_eq!(out, "z");
}

#[test]
fn exec_redirect_only_applies_to_shell() {
    let path = std::env::temp_dir().join(format!("fdshell_exec_redirect_{}", std::process::id()));
    let script = format!("exec 1>{}; printf hi", path.display());
    let (out, _err, code) = run(&script);
    assert_eq!(code, 0);
    assert_eq!(out, "");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hi");
}

#[test]
fn eval_assignment_persists_in_shell() {
    let (out, _err, code) = run(r#"eval "X=1"; printf "$X""#);
    assert_eq!(code, 0);
    assert_eq!(out, "1");
}

#[test]
fn eval_joins_args_with_spaces() {
    let (out, err, code) = run("eval printf a b");
    assert_eq!(code, 0, "args must be space-joined: {err}");
    assert_eq!(out, "a");
}

#[test]
fn eval_double_expansion() {
    let (out, _err, code) = run(r#"S="printf hi"; eval "$S""#);
    assert_eq!(code, 0);
    assert_eq!(out, "hi");
}

#[test]
fn eval_runs_conditional_and_keeps_status() {
    let (out, _err, code) = run(r#"eval "true && printf both"; eval "test 1 -eq 2"; printf "s$?""#);
    assert_eq!(code, 0);
    assert_eq!(out, "boths1");
}

#[test]
fn eval_parse_error_fails_script() {
    let (_out, err, code) = run(r#"eval "if""#);
    assert_ne!(code, 0);
    assert!(!err.is_empty());
}

#[test]
fn eval_break_exits_loop() {
    let (out, _err, code) = run(r#"while true; do eval "break"; done; printf done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "done");
}

#[test]
fn eval_continue_skips_iteration() {
    let (out, _err, code) = run(r#"for x in a b; do eval "continue"; printf X; done; printf done"#);
    assert_eq!(code, 0);
    assert_eq!(out, "done");
}

#[test]
fn eval_break_outside_loop_errors() {
    let (_out, err, code) = run(r#"eval "break""#);
    assert_ne!(code, 0);
    assert!(!err.is_empty());
}

#[test]
fn param_expansion_indirect() {
    let (out, _err, code) = run(r#"X=hello; n=X; printf "${!n}""#);
    assert_eq!(code, 0);
    assert_eq!(out, "hello");
}

#[test]
fn param_expansion_dash_and_plus_colon() {
    let (out, _err, code) =
        run(r#"X=; printf "${X:-d}"; X=hi; printf "${X:-d} ${X:+a}"; printf "${Y:+a}""#);
    assert_eq!(code, 0);
    assert_eq!(out, "dhi a");
}

#[test]
fn param_expansion_assign_colon() {
    let (out, _err, code) = run(r#"printf "${Z:=v} [$Z]""#);
    assert_eq!(code, 0);
    assert_eq!(out, "v [v]");
}

#[test]
fn param_expansion_question_colon_fails_command() {
    let (out, err, code) = run(r#"printf "a${NOPE:?boom}b""#);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("NOPE: boom"), "got: {err}");
}

#[test]
fn exec_with_command_and_redirect_still_execs() {
    let path =
        std::env::temp_dir().join(format!("fdshell_exec_cmd_redirect_{}", std::process::id()));
    let script = format!("exec printf abc 1>{}", path.display());
    let (out, _err, code) = run(&script);
    assert_eq!(code, 0);
    assert_eq!(out, "");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "abc");
}

#[test]
fn exec_with_command_no_redirect_becomes() {
    let (out, _err, code) = run("exec printf became");
    assert_eq!(code, 0);
    assert_eq!(out, "became");
}

#[test]
fn exec_stdin_redirect_feeds_read() {
    let in_path =
        std::env::temp_dir().join(format!("fdshell_exec_stdin_in_{}", std::process::id()));
    std::fs::write(&in_path, "abc\n").unwrap();
    let script = format!("exec <{}; read v; printf \"$v\"", in_path.display());
    let (out, _err, code) = run(&script);
    assert_eq!(code, 0);
    assert_eq!(out, "abc");
}

#[test]
fn test_integer_comparison() {
    let (out, _err, code) = run("if test 10 -gt 9; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn test_malformed_expression_exits_2() {
    let (out, err, code) = run("test a b c d; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
    assert!(err.contains("test"), "stderr={err:?}");
}

#[test]
fn bracket_missing_closer_exits_2() {
    let (out, err, code) = run("[ -f /tmp; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
    assert!(err.contains("test"), "stderr={err:?}");
}

#[test]
fn bracket_fd_var_operand() {
    let (out, _err, code) = run("cd /tmp; if [ -d %CWD ]; then printf y; else printf n; fi");
    assert_eq!(code, 0);
    assert_eq!(out, "y");
}

#[test]
fn bare_test_is_false() {
    let (out, _err, code) = run("test; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "1\n");
}

#[test]
fn bracket_wrong_closer_exits_2() {
    let (out, err, code) = run("[ -f /tmp foo; echo $?");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
    assert!(err.contains("test"), "stderr={err:?}");
}

#[test]
fn test_in_conditional_list() {
    let (out, _err, code) = run("test 1 -eq 1 && printf a; test 1 -eq 2 || printf b");
    assert_eq!(code, 0);
    assert_eq!(out, "ab");
}

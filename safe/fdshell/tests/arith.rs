#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::str;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

fn run(script: &str) -> std::process::Output {
    Command::new(BIN)
        .args(["-c", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap()
}

/// `$((…))` is evaluated in-process and replaced with its decimal result;
/// operators keep C precedence (`*` before `+`).
#[test]
fn arith_expansion_uses_c_precedence() {
    let output = run("echo $((2+3*4))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "14");
}

/// Assignment through `$((x+1))` on the right-hand side: the value is
/// expanded before the variable is set.
#[test]
fn arith_expansion_feeds_assignment() {
    let output = run("x=1; x=$((x+1)); echo $x");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "2");
}

/// Assignment inside the expression returns the new value and stores it,
/// so a later word on the same line sees the update.
#[test]
fn arith_internal_assignment_returns_and_stores() {
    let output = run("echo $((y=3)) $y");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "3 3");
}

/// The ternary picks the taken branch.
#[test]
fn arith_ternary_branches() {
    let t = run("echo $((1?2:3))");
    assert!(t.status.success());
    assert_eq!(str::from_utf8(&t.stdout).unwrap().trim(), "2");
    let f = run("echo $((0?2:3))");
    assert!(f.status.success());
    assert_eq!(str::from_utf8(&f.stdout).unwrap().trim(), "3");
}

/// An unset variable in arithmetic is 0 (no error, no word-splitting).
#[test]
fn arith_unset_variable_is_zero() {
    let output = run("echo $((unsetv+1))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "1");
}

/// `$((…))` inside a quoted word expands in place, keeping the word whole.
#[test]
fn arith_expansion_inside_quoted_word() {
    let output = run("echo \"a$((1+1))b\"");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "a2b");
}

/// Hex and leading-0 octal literals, like bash.
#[test]
fn arith_hex_and_octal_literals() {
    let output = run("echo $((0xff)) $((010))");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "255 8");
}

/// `base#number` literals (POSIX #5.3), like bash: `16#ff` is 255. The
/// `((…))` keyword form shares the lexer, and a digit outside the base is a
/// clean syntax error.
#[test]
fn arith_radix_literals() {
    let output = run("echo $((16#ff)) $((2#1010)) $((8#17)) $((36#zz))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(
        str::from_utf8(&output.stdout).unwrap().trim(),
        "255 10 15 1295"
    );

    let bad = run("echo $((2#2))");
    let stderr = str::from_utf8(&bad.stderr).unwrap();
    assert!(
        stderr.contains("arithmetic expression has a syntax error"),
        "stderr={stderr}"
    );
    assert!(!bad.status.success());

    let kw = run("((16#ff)); echo $?");
    assert!(kw.status.success());
    assert_eq!(str::from_utf8(&kw.stdout).unwrap().trim(), "0");
}

/// Compound assignment returns the new value and updates the variable.
#[test]
fn arith_compound_assignment_updates_variable() {
    let output = run("x=7; echo $((x+=3)) $x");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "10 10");
}

/// A whole-word `$((…))` in a `for … in` list expands to its value, and the
/// body runs once per expanded word.
#[test]
fn arith_expansion_in_for_list() {
    let output = run("for i in $((1+2)) $((3*3)); do echo $i; done");
    assert!(output.status.success());
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "3\n9");
}

// --- in-body substitution: `$(…)` and nested `$((…))` ---

/// `$(…)` inside a `$((…))` body runs a child and splices its output, which is
/// re-parsed as part of the arithmetic expression.
#[test]
fn arith_cmd_subst_in_body() {
    let output = run("echo $(( $(echo 1) + 1 ))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "2");
}

/// A nested `$((…))` inside a `$((…))` body is evaluated and spliced in place.
#[test]
fn arith_nested_arith_in_body() {
    let output = run("echo $(( $((1+2)) * 3 ))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "9");
}

/// A whole-word `$(( $(…)))` in a `for … in` list expands through the for-list
/// path (the word arrives with both closers intact).
#[test]
fn arith_cmd_subst_in_for_list() {
    let output = run("for i in $(( $(echo 1) + 1 )); do echo $i; done");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "2");
}

/// Command output as an operand: the ternary picks the taken branch.
#[test]
fn arith_cmd_subst_as_operand() {
    let output = run("echo $(( $(echo 7) > 2 ? 10 : 20 ))");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "10");
}

/// Exceeding the capture limit inside an arithmetic substitution is a clean
/// error that names both the arithmetic substitution and the limit.
#[test]
fn arith_cmd_subst_capture_limit_fails() {
    let output = run("set --stdout-capture-limit 3; echo $(( $(printf abcd) + 1 ))");
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("arithmetic"), "stderr={stderr}");
    assert!(stderr.contains("capture limit"), "stderr={stderr}");
    assert!(!output.status.success());
}

/// `$(…)` inside a `$((…))` also works in a double-quoted word.
#[test]
fn arith_cmd_subst_in_quoted_word() {
    let output = run("echo \"$(( $(echo 1) + 1 ))\"");
    assert!(
        output.status.success(),
        "stderr={}",
        str::from_utf8(&output.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "2");
}

/// Division by zero is a clean error that fails the command.
#[test]
fn arith_division_by_zero_fails_command() {
    let output = run("echo $((1/0))");
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("division or modulo by zero"),
        "stderr={stderr}"
    );
    assert!(!output.status.success());
}

/// A malformed expression is a clean syntax error that fails the command.
#[test]
fn arith_malformed_expression_fails_command() {
    let output = run("echo $((1+))");
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("arithmetic expression has a syntax error"),
        "stderr={stderr}"
    );
    assert!(!output.status.success());
}

// --- `((expr))` arithmetic command (keyword) ---

/// `((expr))` is a shell keyword: it evaluates `expr` in-process and sets the
/// exit status to `expr == 0` (0 when the value is non-zero, 1 when it is 0).
#[test]
fn arith_command_sets_exit_status() {
    let ok = run("((1+2)); echo $?");
    assert!(
        ok.status.success(),
        "stderr={}",
        str::from_utf8(&ok.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&ok.stdout).unwrap().trim(), "0");

    let zero = run("((0)); echo $?");
    assert_eq!(str::from_utf8(&zero.stdout).unwrap().trim(), "1");
}

/// `((…))` participates in `&&` / `||` cond lists.
#[test]
fn arith_command_in_cond_list() {
    let big = run("x=5; ((x>3)) && echo big");
    assert!(big.status.success());
    assert_eq!(str::from_utf8(&big.stdout).unwrap().trim(), "big");

    let no = run("((0)) || echo no");
    assert_eq!(str::from_utf8(&no.stdout).unwrap().trim(), "no");
}

/// `||` / `&&` inside the expression belong to the expression, not the cond
/// list (the cond-list splitter must not cut them).
#[test]
fn arith_command_inner_logical_operators() {
    let or = run("((1||0)); echo $?");
    assert!(
        or.status.success(),
        "stderr={}",
        str::from_utf8(&or.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&or.stdout).unwrap().trim(), "0");

    let and = run("((1&&1)); echo $?");
    assert!(and.status.success());
    assert_eq!(str::from_utf8(&and.stdout).unwrap().trim(), "0");
}

/// `|` inside the expression is a bitwise or, not a pipeline split.
#[test]
fn arith_command_inner_pipe_is_bitwise() {
    let pipe = run("((3|4)); echo $?");
    assert!(
        pipe.status.success(),
        "stderr={}",
        str::from_utf8(&pipe.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&pipe.stdout).unwrap().trim(), "0");

    let var = run("x=6; ((x|1)); echo $?");
    assert!(var.status.success());
    assert_eq!(str::from_utf8(&var.stdout).unwrap().trim(), "0");
}

/// An assignment inside the expression updates the variable (side effect).
#[test]
fn arith_command_assignment_side_effect() {
    let out = run("x=2; ((x=3)); echo $x");
    assert!(out.status.success());
    assert_eq!(str::from_utf8(&out.stdout).unwrap().trim(), "3");
}

// --- `let` builtin ---

/// `let` evaluates each argument as an arithmetic expression; the exit status
/// comes from the last one.
#[test]
fn let_builtin_evaluates_and_sets_status() {
    let val = run("let x=3+4; echo $x");
    assert!(
        val.status.success(),
        "stderr={}",
        str::from_utf8(&val.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&val.stdout).unwrap().trim(), "7");

    let ok = run("let x=3+4; echo $?");
    assert_eq!(str::from_utf8(&ok.stdout).unwrap().trim(), "0");

    let zero = run("let 0; echo $?");
    assert_eq!(str::from_utf8(&zero.stdout).unwrap().trim(), "1");
}

/// `let` takes several expressions and a quoted single-argument form.
#[test]
fn let_builtin_multiple_and_quoted_args() {
    let multi = run("let i=1 j=2; echo $i$j");
    assert!(multi.status.success());
    assert_eq!(str::from_utf8(&multi.stdout).unwrap().trim(), "12");

    let quoted = run("let \"a = 2 + 3\"; echo $a");
    assert!(quoted.status.success());
    assert_eq!(str::from_utf8(&quoted.stdout).unwrap().trim(), "5");
}

/// `let` with no argument is a clean "expression expected" error.
#[test]
fn let_builtin_no_args_is_error() {
    let out = run("let");
    let stderr = str::from_utf8(&out.stderr).unwrap();
    assert!(stderr.contains("expression expected"), "stderr={stderr}");
    assert!(!out.status.success());
}

/// A malformed expression in `((…))` or `let` is a clean syntax error.
#[test]
fn arith_command_malformed_expression_is_error() {
    let cmd = run("((1+))");
    let stderr = str::from_utf8(&cmd.stderr).unwrap();
    assert!(
        stderr.contains("arithmetic expression has a syntax error"),
        "stderr={stderr}"
    );
    assert!(!cmd.status.success());

    let let_out = run("let 1+");
    let let_stderr = str::from_utf8(&let_out.stderr).unwrap();
    assert!(
        let_stderr.contains("arithmetic expression has a syntax error"),
        "stderr={let_stderr}"
    );
    assert!(!let_out.status.success());
}

/// `((…))` works as a block condition (`while` / `if`); the body round-trips.
#[test]
fn arith_command_as_block_condition() {
    let loop_out = run("i=0; while ((i<3)); do i=$((i+1)); done; echo $i");
    assert!(
        loop_out.status.success(),
        "stderr={}",
        str::from_utf8(&loop_out.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&loop_out.stdout).unwrap().trim(), "3");

    let if_out = run("if ((1)); then echo t; fi");
    assert!(if_out.status.success());
    assert_eq!(str::from_utf8(&if_out.stdout).unwrap().trim(), "t");
}

/// A `#` comment after a `((…))` command is stripped before the parse.
#[test]
fn arith_command_with_trailing_comment() {
    let out = run("((1)) # comment");
    assert!(
        out.status.success(),
        "stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap().trim(), "");
}

/// `((x)) foo` is a clean parse error that mentions the arithmetic form.
#[test]
fn arith_command_trailing_words_is_parse_error() {
    let out = run("((x)) foo");
    let stderr = str::from_utf8(&out.stderr).unwrap();
    assert!(stderr.contains("arithmetic command"), "stderr={stderr}");
    assert!(!out.status.success());
}

/// `builtin let` bypasses the lookup and runs the `let` intercept.
#[test]
fn builtin_prefix_runs_let() {
    let out = run("builtin let x=1; echo $x");
    assert!(
        out.status.success(),
        "stderr={}",
        str::from_utf8(&out.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&out.stdout).unwrap().trim(), "1");
}

/// Depth-fix regression: `&&` / `||` inside `$(…)` / `$((…))` no longer split
/// the statement (these failed before the paren-depth scan).
#[test]
fn paren_depth_keeps_substitution_intact() {
    let or = run("x=0; y=5; echo $((x||y))");
    assert!(
        or.status.success(),
        "stderr={}",
        str::from_utf8(&or.stderr).unwrap()
    );
    assert_eq!(str::from_utf8(&or.stdout).unwrap().trim(), "1");

    let cmd = run("echo $(echo a && echo b)");
    let stdout = str::from_utf8(&cmd.stdout).unwrap();
    assert!(
        cmd.status.success(),
        "stderr={}",
        str::from_utf8(&cmd.stderr).unwrap()
    );
    assert!(
        stdout.contains('a') && stdout.contains('b'),
        "stdout={stdout}"
    );
}

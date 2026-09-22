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
fn heredoc_feeds_stdin() {
    let (out, _err, code) = run("cat <<EOF\nline1\nline2\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "line1\nline2\n");
}

#[test]
fn heredoc_body_has_no_extra_trailing_newline() {
    let (out, _err, code) = run("wc -c <<EOF\nab\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "3\n");
}

#[test]
fn heredoc_empty_body_is_zero_bytes() {
    let (out, _err, code) = run("wc -c <<EOF\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
}

#[test]
fn heredoc_counts_lines() {
    let (out, _err, code) = run("wc -l <<EOF\na\nb\nc\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "3\n");
}

#[test]
fn heredoc_unquoted_delimiter_expands() {
    let (out, _err, code) = run("X=zzz; cat <<Q\n$X\nQ");
    assert_eq!(code, 0);
    assert_eq!(out, "zzz\n");
}

#[test]
fn heredoc_quoted_delimiter_is_literal() {
    let (out, _err, code) = run("X=zzz; cat <<\"Q\"\n$X\nQ");
    assert_eq!(code, 0);
    assert_eq!(out, "$X\n");
}

#[test]
fn heredoc_bare_form_takes_next_word() {
    let (out, _err, code) = run("cat << Q\nbody\nQ");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n");
}

#[test]
fn heredoc_keeps_other_args_intact() {
    let (out, _err, code) = run("echo a <<EOF b\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "a b\n");
}

#[test]
fn heredoc_near_miss_delimiter_line_is_body() {
    let (out, _err, code) = run("cat <<EOF\nxEOF\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "xEOF\n");
}

#[test]
fn heredoc_pipeline_stage() {
    let (out, _err, code) = run("cat <<EOF | wc -l\na\nb\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
}

#[test]
fn heredoc_in_if_body() {
    let (out, _err, code) = run("if true; then cat <<EOF\nin-if\nEOF\nfi");
    assert_eq!(code, 0);
    assert_eq!(out, "in-if\n");
}

#[test]
fn heredoc_in_while_body_two_iterations() {
    // The trailing `printf` normalizes the loop's exit status.
    let (out, _err, code) = run(
        "i=1; while [ \"$i\" != \"3\" ]; do cat <<EOF\niter $i\nEOF\ni=$((i+1)); done; printf ok",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "iter 1\niter 2\nok");
}

#[test]
fn heredoc_in_function_body() {
    let (out, _err, code) = run("f() { cat <<EOF\nin-func\nEOF\n}; f");
    assert_eq!(code, 0);
    assert_eq!(out, "in-func\n");
}

#[test]
fn heredoc_in_case_clause() {
    let (out, _err, code) = run("case x in x) cat <<EOF\nin-case\nEOF\n;; esac");
    assert_eq!(code, 0);
    assert_eq!(out, "in-case\n");
}

#[test]
fn heredoc_body_and_is_opaque_in_cond_list() {
    let (out, _err, code) = run("cat <<EOF\nx && y\nEOF\necho after");
    assert_eq!(code, 0);
    assert_eq!(out, "x && y\nafter\n");
}

#[test]
fn heredoc_body_comment_and_substitution_text_is_literal() {
    // A quoted delimiter makes the whole body literal: `#` and `$(…)` are
    // never comments or substitutions.
    let (out, _err, code) = run("cat <<\"Q\"\n# not a comment\n$(not run)\nQ");
    assert_eq!(code, 0);
    assert_eq!(out, "# not a comment\n$(not run)\n");
}

#[test]
fn heredoc_body_keyword_lines_do_not_close_block() {
    let (out, _err, code) = run("if true; then cat <<EOF\nfi\ndone\nEOF\nfi; echo after");
    assert_eq!(code, 0);
    assert_eq!(out, "fi\ndone\nafter\n");
}

#[test]
fn heredoc_statement_after_body_runs() {
    let (out, _err, code) = run("cat <<EOF\nbody\nEOF\necho after");
    assert_eq!(code, 0);
    assert_eq!(out, "body\nafter\n");
}

#[test]
fn heredoc_operator_semicolon_is_error() {
    let (_out, err, code) = run("cat <<EOF ;\nEOF");
    assert_ne!(code, 0);
    assert!(err.contains("here-doc"), "stderr={err:?}");
}

#[test]
fn heredoc_bare_operator_at_eol_is_error() {
    let (_out, err, code) = run("cat <<");
    assert_ne!(code, 0);
    assert!(!err.is_empty(), "stderr must carry the error");
}

#[test]
fn heredoc_missing_delimiter_is_error() {
    let (_out, err, code) = run("cat <<NOPE\nbody\nnothing");
    assert_ne!(code, 0);
    assert!(err.contains("terminating"), "stderr={err:?}");
}

#[test]
fn heredoc_in_comment_is_not_an_operator() {
    // A `<<EOF` inside a comment is comment text: it must not open a heredoc
    // or swallow the following `EOF`-shaped line, which stays a real command.
    let (out, err, code) = run("echo hi # <<EOF\nEOF");
    assert_eq!(out, "hi\n");
    assert_ne!(code, 0);
    assert!(err.contains("\"EOF\" not found"), "stderr={err:?}");
}

#[test]
fn heredoc_empty_quoted_delimiter() {
    // `<<""` (empty quoted delimiter): the body ends at the first blank line.
    let (out, _err, code) = run("cat <<\"\"\nbody\n\necho [AFTER]");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n[AFTER]\n");
}

#[test]
fn heredoc_empty_quoted_delimiter_byte_count() {
    let (out, _err, code) = run("wc -c <<\"\"\nbody\n\nprintf [AFTER]");
    assert_eq!(code, 0);
    assert_eq!(out, "5\n[AFTER]");
}

#[test]
fn heredoc_empty_quoted_delimiter_empty_body() {
    // The blank line directly after the command line: zero-byte body.
    let (out, _err, code) = run("wc -c <<\"\"\n\nprintf [AFTER]");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n[AFTER]");
}

#[test]
fn heredoc_empty_quoted_delimiter_eof_is_error() {
    // EOF right after the body: no blank line, so the delimiter line is
    // missing. Explicit error (bash only warns); never a silent cut.
    let (_out, err, code) = run("wc -c <<\"\"\nbody\n");
    assert_ne!(code, 0);
    assert!(err.contains("terminating"), "stderr={err:?}");
}

#[test]
fn heredoc_attached_after_substitution_fails_explicitly() {
    // A `$( )`-closing `)` does not break the word, so `$(true)<<EOF` is one
    // argument: `<<EOF` is a file `cat` cannot open, and the body lines are
    // real commands. Explicit errors, never a silent swallow.
    let (out, err, code) = run("cat $(true)<<EOF\nBODY\nEOF\necho AFTER");
    assert_eq!(code, 0);
    assert_eq!(out, "AFTER\n");
    assert!(err.contains("\"BODY\" not found"), "stderr={err:?}");
    assert!(err.contains("\"EOF\" not found"), "stderr={err:?}");
}

#[test]
fn heredoc_pipe_position_fails_explicitly() {
    // After a `|`, `<<EOF` is a command word, not an operator: the body lines
    // stay real commands. Explicit errors, never a silent swallow.
    let (_out, err, code) = run("true | <<EOF\nx\nEOF");
    assert_ne!(code, 0);
    assert!(err.contains("\"x\" not found"), "stderr={err:?}");
    assert!(err.contains("\"EOF\" not found"), "stderr={err:?}");
}

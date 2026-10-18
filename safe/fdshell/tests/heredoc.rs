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

/// Run with the shell's fd 3 **closed from the start**: `sh` closes it before
/// `exec`-ing fdshell, so the fd 3 the child sees is deterministic whatever
/// the harness inherited (cargo-mutants hands every test process its
/// jobserver pipes as open fds ≥ 3 — LESSONS). A pin that asserts the
/// parent's fd 3 is *closed* through this helper; one that occupies fd 3
/// opens a pipe and passes its read end as fd 3.
fn run_fd3_closed(script: &str) -> (String, String, i32) {
    let output = Command::new("sh")
        .args(["-c", "exec 3>&-; exec \"$0\" -c \"$1\"", BIN, script])
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

// --- `<<-` tab-stripping form ----------------------------------------------

#[test]
fn heredoc_dash_form_strips_leading_tabs() {
    let (out, _err, code) = run("cat <<-EOF\n\tone\n\t\ttwo\nthree\n\tEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "one\ntwo\nthree\n");
}

#[test]
fn heredoc_dash_form_keeps_spaces_and_midline_tabs() {
    // Leading spaces are never stripped; a mid-line tab is kept.
    let (out, _err, code) = run("cat <<-EOF\n  sp\n\ttab\n\t\ta\tb\n\tEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "  sp\ntab\na\tb\n");
}

#[test]
fn heredoc_dash_quoted_delimiter_is_literal_and_stripped() {
    let (out, _err, code) = run("X=v; cat <<-\"Q\"\n\t$X\n\tQ");
    assert_eq!(code, 0);
    assert_eq!(out, "$X\n");
}

#[test]
fn heredoc_dash_unquoted_delimiter_expands() {
    let (out, _err, code) = run("X=v; cat <<-Q\n\t$X\n\tQ");
    assert_eq!(code, 0);
    assert_eq!(out, "v\n");
}

#[test]
fn heredoc_dash_separate_word_strips() {
    let (out, _err, code) = run("cat <<- EOF\n\tbody\n\tEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n");
}

#[test]
fn heredoc_dash_form_byte_count_no_trailing_newline() {
    // The stripped body is `ab\n`: three bytes, no extra newline appended.
    let (out, _err, code) = run("wc -c <<-EOF\n\tab\n\tEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "3\n");
}

#[test]
fn heredoc_plain_form_ignores_tab_indented_terminator() {
    // The plain `<<` form never strips tabs, so `\tEOF` is not the terminator.
    let (_out, err, code) = run("cat <<EOF\n\tEOF");
    assert_ne!(code, 0);
    assert!(err.contains("terminating"), "stderr={err:?}");
}

#[test]
fn heredoc_dash_bare_operator_at_eol_is_error() {
    let (_out, err, code) = run("cat <<-");
    assert_ne!(code, 0);
    assert!(!err.is_empty(), "stderr must carry the error");
}

#[test]
fn heredoc_dash_in_if_body() {
    let (out, _err, code) = run("if true; then cat <<-E\n\tin-if\n\tE\nfi");
    assert_eq!(code, 0);
    assert_eq!(out, "in-if\n");
}

#[test]
fn heredoc_dash_pipeline_stage() {
    let (out, _err, code) = run("cat <<-E | wc -l\n\ta\n\tb\n\tE");
    assert_eq!(code, 0);
    assert_eq!(out, "2\n");
}

#[test]
fn heredoc_dash_body_is_opaque() {
    // `&&` and `;` in the body are content, not statement structure.
    let (out, _err, code) = run("cat <<-E\n\tx && y\n\t;\n\tE\necho after");
    assert_eq!(code, 0);
    assert_eq!(out, "x && y\n;\nafter\n");
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
fn heredoc_after_semicolon_word_drives_boundary_skip() {
    // A `;`-separated word with redirect-like bytes before the here-doc: the
    // here-doc — not the `>`-shaped word bytes — drives the boundary skip, so
    // the `&&` in the body stays opaque to the cond-list split.
    let (out, _err, code) = run("echo a>/dev/null; cat <<EOF\nbody && x\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "a>/dev/null\nbody && x\n");
}

#[test]
fn heredoc_attached_operator_before_semicolon_is_not_a_delimiter() {
    // The `;` resets the run start, so the here-doc's operator search window
    // is only `cat <<EOF`: the attached `x<<A` before the `;` is a word, not
    // a here-doc operator, and the body (with its `&&`) is skipped as one
    // opaque part.
    let (out, _err, code) = run("echo a>b x<<A; cat <<EOF\nbody && x\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "a>b x<<A\nbody && x\n");
}

#[test]
fn block_heredoc_cond_run_and_and_in_body_splits_part() {
    // A `;`-terminated `<<` run inside a block condition must not widen the
    // heredoc-skip window: the `&&` in the body line splits the part, so the
    // if-parse reports the missing `fi`, not a heredoc error.
    let (_out, err, code) = run("if cat <<X; then\nbody && x\nX\nfi");
    assert_eq!(code, 1);
    assert!(err.contains("missing 'fi'"));
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
fn heredoc_operator_semicolon_reads_body_after_line() {
    // A `;` after the operator: the body is read after the whole logical
    // line (bash-compatible).
    let (out, _err, code) = run("cat <<EOF ;\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "");
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

// --- A line that opens with a redirect/separator token: the `<<` is an
// operator at both the byte and the token level (the byte scan needs a preceding
// *token*, not a preceding word — task #185), so the scanner claims the body
// region and the statement fails on its operator word. The body lines are
// consumed, never executed. bash is a rc-2 syntax error on every line here;
// fdshell maps it to its rc-1 "not found" family. ---

/// A leading bare `>`: the statement's command word is `>`, its body is consumed.
#[test]
fn heredoc_leading_redirect_operator_consumes_its_body() {
    let (out, err, code) = run("> <<EOF\nhi\nEOF");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("\">\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"hi\" not found"),
        "body must not leak: {err:?}"
    );
    assert!(
        !err.contains("\"EOF\" not found"),
        "body must not leak: {err:?}"
    );
}

/// The same with the POSIX #2.2 clobber operator.
#[test]
fn heredoc_leading_clobber_operator_consumes_its_body() {
    let (out, err, code) = run(">| <<EOF\nhi\nEOF");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("\">|\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"hi\" not found"),
        "body must not leak: {err:?}"
    );
    assert!(
        !err.contains("\"EOF\" not found"),
        "body must not leak: {err:?}"
    );
}

/// Both operators of a `&&` cond-list are allocated in order and the failing
/// first part stops the chain: no body line runs.
#[test]
fn heredoc_leading_operator_in_cond_list_allocates_both_bodies() {
    let (out, err, code) = run("> <<EOF && cat <<A\nhi\nEOF\nx\nA");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("\">\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"hi\" not found"),
        "body must not leak: {err:?}"
    );
    assert!(
        !err.contains("\"x\" not found"),
        "body must not leak: {err:?}"
    );
}

/// Regression guard for the shape the divergence was found on: a word between
/// the clobber operator and the `<<` behaves exactly as before.
#[test]
fn heredoc_clobber_operator_with_word_consumes_its_body() {
    let (out, err, code) = run(">| x <<EOF\nhi\nEOF");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("\">|\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"hi\" not found"),
        "body must not leak: {err:?}"
    );
    assert!(
        !err.contains("\"EOF\" not found"),
        "body must not leak: {err:?}"
    );
}

/// `;` is a token for the tokenizer, so the `<<` after it counts and the body
/// is consumed: master ran `hi` and `EOF` as commands, the fixed scan does not.
#[test]
fn heredoc_after_semicolon_with_no_command_word_consumes_its_body() {
    let (out, err, code) = run("; <<EOF\nhi\nEOF");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("\"<<EOF\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"hi\" not found"),
        "body must not leak: {err:?}"
    );
    assert!(
        !err.contains("\"EOF\" not found"),
        "body must not leak: {err:?}"
    );
}

// --- New positions: heredoc operators in cond-list and block-condition
// positions (bash-compatible: body read after the whole logical line, in
// operator order). ---

#[test]
fn heredoc_cond_and_position() {
    // `<<EOF && true`: the body is read after the whole logical line.
    let (out, _err, code) = run("cat <<EOF && true\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n");
}

#[test]
fn heredoc_cond_or_position() {
    // `false || cat <<EOF`: the body is read after the whole logical line.
    let (out, _err, code) = run("false || cat <<EOF\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n");
}

#[test]
fn heredoc_semicolon_position() {
    // `cat <<EOF; echo done`: the body is read after the whole logical line.
    let (out, _err, code) = run("cat <<EOF; echo done\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "body\ndone\n");
}

#[test]
fn heredoc_if_condition_position() {
    // `if cat <<EOF; then`: the body is read after the whole logical line.
    let (out, _err, code) = run("if cat <<EOF; then echo [THEN]; fi\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n[THEN]\n");
}

#[test]
fn heredoc_until_condition_position() {
    // `until cat <<EOF; do`: the body is read after the whole logical line.
    // (`cat` exits 0, so the until body never runs; a `while` variant would
    // loop forever on a successful condition, and `&&` in a loop condition
    // is a separate pre-existing limitation.)
    let (out, _err, code) = run("until cat <<EOF; do echo [BODY]; done\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n");
}

#[test]
fn heredoc_multi_operator_ordering() {
    // Multiple operators across `;` and `&&`: bodies are read in operator
    // order.
    let (out, _err, code) = run("cat <<A && cat <<B; cat <<C\nbodyA\nA\nbodyB\nB\nbodyC\nC");
    assert_eq!(code, 0);
    assert_eq!(out, "bodyA\nbodyB\nbodyC\n");
}

#[test]
fn heredoc_body_contains_other_delimiter() {
    // A body may contain another operator's delimiter line: the delimiters
    // are matched in operator order, so `B` inside A's body is data for A,
    // and B's own body (after A's delimiter) is empty.
    let (out, _err, code) = run("cat <<A && cat <<B\nB\nA\nB");
    assert_eq!(code, 0);
    assert_eq!(out, "B\n");
}

#[test]
fn heredoc_elif_condition_position() {
    // `elif cat <<EOF; then`: the body is read after the whole logical line,
    // before the taken arm runs.
    let (out, _err, code) =
        run("if false; then echo a; elif cat <<EOF; then echo b; fi\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "body\nb\n");
}

// --- Heredoc statements adjacent to blocks on the same line: a block must
// neither swallow a trailing heredoc statement nor hand its own body regions
// to the statements around it. ---

#[test]
fn heredoc_stmt_after_if_block_same_line() {
    // The trailing `; cat <<EOF` is a real statement, not part of the `if`
    // text: the block ends at `fi;` and the heredoc runs after it.
    let (out, _err, code) = run("if true; then echo y; fi; cat <<EOF\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "y\nbody\n");
}

#[test]
fn heredoc_stmt_after_while_block_same_line() {
    let (out, _err, code) = run("while true; do echo a; break; done; cat <<EOF\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "a\nbody\n");
}

#[test]
fn heredoc_stmt_after_for_block_same_line() {
    let (out, _err, code) = run("for x in a b; do echo $x; done; cat <<EOF\nbody\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "a\nb\nbody\n");
}

#[test]
fn heredoc_if_condition_and_trailing_stmt_same_line() {
    // A condition-position heredoc and a trailing heredoc statement on one
    // line: the condition takes the first body region, the trailing
    // statement the second (operator order).
    let (out, _err, code) = run("if cat <<A; then echo y; fi; cat <<B\nbodyA\nA\nbodyB\nB");
    assert_eq!(code, 0);
    assert_eq!(out, "bodyA\ny\nbodyB\n");
}

#[test]
fn heredoc_stmt_before_if_block_same_line() {
    // A `;`-terminated heredoc statement before the block on the same line:
    // it runs first with its own body; the block's line starts after it.
    let (out, _err, code) = run("cat <<A; if true; then echo y; fi\nbody\nA");
    assert_eq!(code, 0);
    assert_eq!(out, "body\ny\n");
}

#[test]
fn heredoc_stmt_before_function_block_same_line() {
    let (out, _err, code) = run("cat <<A; f() { echo hi; }; f\nbody\nA");
    assert_eq!(code, 0);
    assert_eq!(out, "body\nhi\n");
}

// --- All three adjacencies on one line: a `;`-terminated heredoc statement
// before a block, a condition-position heredoc in the block, and a heredoc
// statement after it. The condition must be fed its OWN body region (the
// regions are allocated per physical line, in operator order). ---

#[test]
fn heredoc_stmt_before_if_condition_and_after_same_line() {
    // bash: `bodyX bodyA y bodyB`.
    let (out, _err, code) =
        run("cat <<X; if cat <<A; then echo y; fi; cat <<B\nbodyX\nX\nbodyA\nA\nbodyB\nB");
    assert_eq!(code, 0);
    assert_eq!(out, "bodyX\nbodyA\ny\nbodyB\n");
}

#[test]
fn heredoc_two_stmts_before_if_condition_same_line() {
    // Two pre-block operators: the condition takes region 2 of the line.
    let (out, _err, code) = run(
        "cat <<X; cat <<Y; if cat <<A; then echo y; fi; cat <<B\nbodyX\nX\nbodyY\nY\nbodyA\nA\nbodyB\nB",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "bodyX\nbodyY\nbodyA\ny\nbodyB\n");
}

#[test]
fn heredoc_stmt_before_while_condition_and_after_same_line() {
    let (out, _err, code) = run(
        "cat <<X; while cat <<A; do echo w; break; done; cat <<B\nbodyX\nX\nbodyA\nA\nbodyB\nB",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "bodyX\nbodyA\nw\nbodyB\n");
}

#[test]
fn heredoc_stmt_before_until_condition_and_after_same_line() {
    // `cat` exits 0, so `until` stops before its body (bash parity).
    let (out, _err, code) = run(
        "cat <<X; until cat <<A; do echo u; break; done; cat <<B\nbodyX\nX\nbodyA\nA\nbodyB\nB",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "bodyX\nbodyA\nbodyB\n");
}

#[test]
fn heredoc_stmt_before_elif_condition_and_after_same_line() {
    let (out, _err, code) = run(
        "cat <<X; if false; then echo a; elif cat <<E; then echo b; fi; cat <<B\nbodyX\nX\nbodyE\nE\nbodyB\nB",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "bodyX\nbodyE\nb\nbodyB\n");
}

#[test]
fn heredoc_stmt_before_if_condition_no_trailing_stmt_same_line() {
    let (out, _err, code) = run("cat <<X; if cat <<A; then echo y; fi\nbodyX\nX\nbodyA\nA");
    assert_eq!(code, 0);
    assert_eq!(out, "bodyX\nbodyA\ny\n");
}

#[test]
fn heredoc_stmt_before_if_no_condition_heredoc_same_line() {
    // The block claims no region (k = 0); the pre-block statement runs first
    // and the trailing statement takes the region left over.
    let (out, _err, code) = run("cat <<X; if true; then echo y; fi; cat <<B\nbodyX\nX\nbodyB\nB");
    assert_eq!(code, 0);
    assert_eq!(out, "bodyX\ny\nbodyB\n");
}

#[test]
fn heredoc_twice_last_wins() {
    // POSIX #2.5: both bodies are kept in operator order and the last redirect
    // to stdin wins, so `cat` prints the second body. bash 5.3.9 prints `y`.
    let (out, _err, code) = run("cat <<A <<B\nx\nA\ny\nB");
    assert_eq!(code, 0);
    assert_eq!(out, "y\n");
}

#[test]
fn heredoc_bodies_of_one_command_are_read_in_operator_order() {
    // The byte count pins that each `<<` operator consumes its OWN spec: the
    // second body is `yy\n` (3 bytes). A spec walk that re-reads the first spec
    // yields `x\n` (2 bytes).
    let (out, _err, code) = run("wc -c <<A <<B\nx\nA\nyy\nB");
    assert_eq!(code, 0);
    assert_eq!(out, "3\n");
}

/// Task #207: the here-doc delimiter word is a word, so every `"` byte is
/// removed (not one surrounding pair) and any quoting in it makes the body
/// literal. Each expectation is bash's own output for the same script.
#[test]
fn heredoc_delimiter_strips_every_double_quote() {
    for (script, want) in [
        // `<<"a b"` delimits `a b` (the quoted space is delimiter content).
        ("cat <<\"a b\"\na b", ""),
        // `<<a"b"c` delimits `abc`.
        ("cat <<a\"b\"c\nabc", ""),
        // `<<""Q"x"` delimits `Qx` (the pair rule sees only `"` bytes).
        ("cat <<\"\"Q\"x\"\nQx", ""),
        // A quoted delimiter word makes the body literal.
        ("X=v; cat <<a\"b\"c\n$X\nabc", "$X\n"),
        // A pipeline position keeps the same rule.
        ("cat <<\"a b\" | cat\na b", ""),
        // `{a,b}` in the delimiter is protected from brace expansion
        // (LESSONS: protect the terminating delimiter line).
        ("cat <<\"{a,b}\"\n{a,b}", ""),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// Task #207: the single-quote form in the shapes the heredoc layer is
/// sensitive to — a mid-statement operator and the brace-protected terminator.
#[test]
fn heredoc_delimiter_single_quote_forms_in_heredoc_shapes() {
    // bash: `one two three` — the operator is mid-statement.
    let (out, err, code) = run("echo one; cat <<a'b'c\ntwo\nabc\necho three");
    assert_eq!(
        (out.as_str(), code),
        ("one\ntwo\nthree\n", 0),
        "stderr={err:?}"
    );
    // bash: empty body — a quoted brace word is a delimiter, never expanded.
    let (out, err, code) = run("cat <<'{a,b}'\n{a,b}");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // bash: `$X` — the quoted word makes the body literal, terminator `{a,b}`.
    let (out, err, code) = run("X=zzz; cat <<'{a,b}'\n$X\n{a,b}");
    assert_eq!((out.as_str(), code), ("$X\n", 0), "stderr={err:?}");
}

/// Task #323: the bare form's delimiter guard is mask-gated, so a **quoted**
/// delimiter word beginning with an operator byte is a delimiter — bash reads
/// the body (rc 0), the terminator line is the quote-removed word, and the
/// quoting makes the body literal. The `<<-` form keeps its tab-stripping
/// marker (tab-indented body *and* terminator). Unquoted operator-shaped
/// delimiters stay syntax errors
/// (`layout_bare_operator_before_redirect_word_is_invalid`, `cat << <x`).
#[test]
fn heredoc_bare_quoted_operator_delimiter_is_a_delimiter() {
    for (script, want) in [
        ("cat << \"<x\"\nbody\n<x", "body\n"),
        ("cat << \"&x\"\nbody\n&x", "body\n"),
        ("cat << \"%x\"\nbody\n%x", "body\n"),
        ("cat <<- \"<x\"\n\tbody\n\t<x", "body\n"),
        // A quoted delimiter makes the body literal.
        ("X=v; cat << \"<x\"\n$X\n<x", "$X\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

// --- `;`-terminated heredocs in a single-line block body (task #238) -------
//
// A single-line block's text carries its body regions **after** the closing
// keyword (append mode), so each arm's text is rebuilt from the regions its own
// `<<` operators claim and parses as a self-contained heredoc. Every
// expectation below is bash 5.3.9's own output for the same script.

#[test]
fn single_line_block_body_heredoc_runs() {
    let (out, err, code) = run("if true; then cat <<T; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

#[test]
fn single_line_loop_body_heredoc_runs() {
    let (out, err, code) = run("while true; do cat <<T; break; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

#[test]
fn single_line_until_body_heredoc_runs() {
    let (out, err, code) = run("until false; do cat <<T; break; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

/// A `while` body that never runs never parses its heredoc: with no delimiter
/// line below, the loop simply produces nothing (bash parity).
#[test]
fn single_line_loop_body_heredoc_of_an_unrun_body_is_silent() {
    let (out, err, code) = run("while false; do cat <<T; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
}

#[test]
fn elif_arm_body_heredoc_runs() {
    let (out, err, code) = run("if false; then :; elif true; then cat <<E; fi\nbodyE\nE");
    assert_eq!((out.as_str(), code), ("bodyE\n", 0), "stderr={err:?}");
}

#[test]
fn else_body_heredoc_runs() {
    let (out, err, code) = run("if false; then :; else cat <<T; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

/// The sharpest ordering shape: the `elif` **condition** and the arm **body**
/// under it sit on one physical line, so the condition claims the region its
/// own operator writes and the arm takes the next one.
#[test]
fn elif_condition_and_arm_body_heredocs_run_in_order() {
    let (out, err, code) =
        run("if false; then :; elif cat <<C; then cat <<T; fi\nbodyC\nC\nbodyT\nT");
    assert_eq!(
        (out.as_str(), code),
        ("bodyC\nbodyT\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn single_line_block_body_two_heredocs_run_in_order() {
    let (out, err, code) = run("if true; then cat <<A; cat <<B; fi\nbodyA\nA\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyA\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn single_line_block_body_heredoc_keeps_trailing_statement() {
    let (out, err, code) = run("if true; then cat <<T; fi; echo after\nbodyT\nT");
    assert_eq!(
        (out.as_str(), code),
        ("bodyT\nafter\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn single_line_block_body_heredoc_keeps_statement_after_the_bodies() {
    let (out, err, code) = run("if true; then cat <<T; fi\nbodyT\nT\necho after");
    assert_eq!(
        (out.as_str(), code),
        ("bodyT\nafter\n", 0),
        "stderr={err:?}"
    );
}

/// The arm's whole text (statements + heredoc) is rebuilt, so the arm's
/// statements and exit status behave as bash's: `false` leaves rc 1.
#[test]
fn single_line_block_body_heredoc_keeps_the_arm_statements() {
    let (out, err, code) = run("if true; then true; cat <<T; false; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 1), "stderr={err:?}");
    let (out, err, code) = run("if true; then cat <<T; fi\nbodyT\nT\necho $?");
    assert_eq!((out.as_str(), code), ("bodyT\n0\n", 0), "stderr={err:?}");
}

#[test]
fn single_line_block_body_heredocs_of_two_blocks_run_in_order() {
    let (out, err, code) =
        run("if true; then cat <<T; fi; if true; then cat <<U; fi\nbodyT\nT\nbodyU\nU");
    assert_eq!(
        (out.as_str(), code),
        ("bodyT\nbodyU\n", 0),
        "stderr={err:?}"
    );
}

/// The block's own trailing pipeline/background form: the block text ends at
/// the closing keyword's separator, so the bodies are appended to the block.
#[test]
fn single_line_block_body_heredoc_survives_pipeline_and_background_forms() {
    let (out, err, code) = run("if true; then cat <<T; fi | cat\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
    let (out, err, code) = run("if true; then cat <<T; fi &\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

/// A body line that is exactly a keyword stays body text at arm granularity
/// (the multi-line `heredoc_body_keyword_lines_do_not_close_block` rule).
#[test]
fn single_line_block_body_heredoc_body_keywords_are_data() {
    let (out, err, code) = run("if true; then cat <<T; fi\nfi\nT");
    assert_eq!((out.as_str(), code), ("fi\n", 0), "stderr={err:?}");
    let (out, err, code) = run("while true; do cat <<T; break; done\ndone\nT");
    assert_eq!((out.as_str(), code), ("done\n", 0), "stderr={err:?}");
    let (out, err, code) = run("if false; then :; else cat <<T; fi\nelse\nT");
    assert_eq!((out.as_str(), code), ("else\n", 0), "stderr={err:?}");
    let (out, err, code) = run("if false; then :; elif true; then cat <<T; fi\nelif\nT");
    assert_eq!((out.as_str(), code), ("elif\n", 0), "stderr={err:?}");
}

/// Body lines stay opaque: `#` is not a comment, and an unquoted delimiter
/// keeps the `$(…)` expansion rule.
#[test]
fn single_line_block_body_heredoc_body_lines_are_opaque() {
    let (out, err, code) = run("if true; then cat <<T; fi\nbody $(echo sub)\nT");
    assert_eq!((out.as_str(), code), ("body sub\n", 0), "stderr={err:?}");
    let (out, err, code) = run("if true; then cat <<T; fi\n# comment\nT");
    assert_eq!((out.as_str(), code), ("# comment\n", 0), "stderr={err:?}");
}

#[test]
fn single_line_block_body_heredoc_survives_quoted_and_dash_delimiters() {
    let (out, err, code) = run("if true; then cat <<\"T\"; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
    let (out, err, code) = run("if true; then cat <<-T; fi\n\tbodyT\n\tT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

/// Task #234: the **appended** body of a single-line block is no longer
/// re-split on its `&&`. An append-mode block hands its body regions to
/// `run_cond_list` as its bodies (like a statement) instead of inlining them
/// in the block text, so the split is the block's own `&&` and the body travels
/// with the part that owns the operator. Measured against bash 5.3.9:
/// `body && x`, rc 0 — fdshell agrees (master was a parse error rc 1).
#[test]
fn single_line_block_body_heredoc_with_and_in_body_runs_the_body() {
    let (out, err, code) = run("if true; then cat <<T; fi\nbody && x\nT");
    assert_eq!((out.as_str(), code), ("body && x\n", 0), "stderr={err:?}");
}

/// Task #234: a cond-list operator written in a heredoc-carrying block's **own
/// command text** (its condition, its body) belongs to the block's cond list,
/// exactly as in the heredoc-free shape — the block text is opaque and only its
/// heredoc **body regions** are scanned for an operator. Measured against bash
/// 5.3.9 on both shapes: `body` `a` `b`, rc 0 (master split the block text and
/// reported `expected 'done'` / `missing 'fi'` rc 1).
#[test]
fn block_with_heredoc_keeps_its_operators_in_its_own_cond_list() {
    let (out, err, code) = run("while cat <<X; do echo a && echo b; break; done\nbody\nX");
    assert_eq!((out.as_str(), code), ("body\na\nb\n", 0), "stderr={err:?}");
    let (out, err, code) = run("if cat <<X; then echo a && echo b; fi\nbody\nX");
    assert_eq!((out.as_str(), code), ("body\na\nb\n", 0), "stderr={err:?}");
}

/// Task #234, the failing-body twin of the pin above: `a && b` is the block's
/// own cond list, `a` is the shell's missing-command report (rc 1, README
/// "Errors"; bash prints `a: command not found` and its `break` leaves rc 0 —
/// the `break`-status divergence of task #262). stdout is what the pin asserts.
#[test]
fn block_with_heredoc_runs_a_failing_body_cond_list() {
    let (out, _err, code) = run("while cat <<X; do a && b; break; done\nbody\nX");
    assert_eq!((out.as_str(), code), ("body\n", 1));
}

/// Task #234: the operator written on a heredoc's **delimiter line** is region
/// data, never a cond-list split. In a multi-line block the body region starts
/// on the block's own lines, so the region swallows `break`/`done` and the
/// block reports its own unterminated heredoc (rc 1) — bash's region rule is
/// the same and ends in a syntax error (rc 2, with its
/// `here-document ... delimited by end-of-file` warning).
#[test]
fn block_body_heredoc_delimiter_line_operator_is_region_data() {
    let (_out, err, code) = run("while :; do\ncat <<Q\nbody\nQ && echo x\nbreak\ndone");
    assert_eq!(code, 1, "bash rc 2");
    assert!(
        err.contains("missing terminating delimiter line 'Q'"),
        "stderr={err:?}"
    );
}

/// Task #234 (review F2): a `#`-comment **block closer written inside a heredoc
/// body region** is inert. The region is skipped whole by the cond-boundary
/// region rule, so its bytes never reach the block-keyword counter and the block
/// closes on its real closer. Measured against bash 5.3.9 on both shapes:
/// `# done` / `# esac`, the body, `a`, `b`, rc 0 — parity gained (master
/// reported `expected 'done'` and `case: missing 'esac'`, rc 1).
#[test]
fn comment_block_closer_in_a_heredoc_body_region_is_inert() {
    let (out, err, code) =
        run("while :; do\ncat <<Q\n# done\nbody\nQ\necho a && echo b\nbreak\ndone");
    assert_eq!(
        (out.as_str(), code),
        ("# done\nbody\na\nb\n", 0),
        "stderr={err:?}"
    );
    let (out, err, code) =
        run("case q in\nq)\ncat <<Q\n# esac\nbody\nQ\necho a && echo b\n;;\nesac");
    assert_eq!(
        (out.as_str(), code),
        ("# esac\nbody\na\nb\n", 0),
        "stderr={err:?}"
    );
}

/// Task #236 (with #237's gap closing on this shape): the same comment written
/// in the block's **own** body line is inert too. The command-position rule
/// (`keywords/position.rs`) puts the words behind a `#` in argument position —
/// the `done` in `# done` is data, the counter stays, the tail `&&` stays
/// inside the body and the loop closes on its real closer. Measured against
/// bash 5.3.9: `body`, `a`, `b`, rc 0 — parity gained (master dropped the
/// counter at the comment's `done`, split the block text at the `&&` and
/// reported `expected 'done'`, rc 1; task #237's comment rule lands with
/// #236's shared one).
#[test]
fn comment_block_closer_in_a_block_body_line_is_inert() {
    let (out, err, code) =
        run("while :; do\ncat <<Q\nbody\nQ\n# done\necho a && echo b\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("body\na\nb\n", 0), "stderr={err:?}");
}

/// Task #234 (review N1): the region scan does not yet discriminate a `<<`
/// written in a block's **clause** (`if cat <<X; then`, the shape whose split
/// ends where bash also errors, rc 2) from one written in the block's **body**:
/// `if :; then cat <<Q` + `z && y` + `Q` + `fi` prints `z && y` in bash (rc 0),
/// as it does on master — the gate switched the block counter off for
/// heredoc-carrying text, so the body line was an ordinary part. The region
/// rule scans every region the block's text carries, so the `&&` in this
/// body-position region splits the block text: rc 1 `missing 'fi'`. Accepted
/// divergence, measured 2026-10-16 (bash 5.3.9 rc 0, master rc 0, this tree
/// rc 1; `case` twin rc 1 `case: missing 'esac'`, `while` twin rc 1 `expected
/// 'done'` where bash loops forever) — task **#304** owns the position gate.
/// When #304 lands, this pin flips to bash parity (`z && y`, rc 0) and drops
/// the `304` suffix from its name.
#[test]
fn and_in_a_body_position_heredoc_region_splits_the_block_until_304() {
    let (_out, err, code) = run("if :; then cat <<Q\nz && y\nQ\nfi");
    assert_eq!(code, 1, "bash rc 0 prints 'z && y'");
    assert!(err.contains("missing 'fi'"), "stderr={err:?}");
}

/// Task #274 / LESSONS "a region window and the skip derived from it are one
/// pair" (measured divergence, pinned as fdshell's own behaviour): the line's
/// regions are allocated for **all** of its operators, so an unterminated
/// `<<E` in the `elif` condition leaves the line with no regions at all and the
/// `then` arm reports its own missing delimiter. bash takes the `then` body and
/// warns about `E`: `bodyT`, rc 0.
#[test]
fn single_line_block_body_heredoc_of_an_incomplete_line_stream_is_empty() {
    let (out, _err, code) = run("if true; then cat <<T; elif cat <<E; then echo b; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("", 1));
}

/// Task #238's out-of-scope interaction, pinned as measured: a block trailing
/// redirect writes the **arm's** stdout, as for a heredoc-free block
/// (`if true; then echo hi; fi > out` behaves identically on master); bash
/// writes the here-doc body into `out` and prints nothing. The silent drop of a
/// block's trailing redirect is the family task **#298** owns. Task #273 makes
/// the `for` and `case` twins *join* that family: on `master` each of them was
/// only a parse error of the untruncated closer search (`expected 'done'` /
/// `case: missing 'esac'`), heredoc-carrying and heredoc-free alike. Pinned:
/// `> out` and its `>>` twin for every block, plus the heredoc-free twins.
#[test]
fn single_line_block_body_heredoc_with_block_trailing_redirect() {
    let dir = std::env::temp_dir().join("fdshell-task238");
    std::fs::create_dir_all(&dir).unwrap();
    // Task #273 makes the `for` and `case` twins of this shape reachable (on
    // `master` they were the parse errors of the untruncated closer search).
    for (script, want) in [
        ("if true; then cat <<T; fi > out\nbodyT\nT", "bodyT\n"),
        ("for x in a; do cat <<T; done > out\nbodyT\nT", "bodyT\n"),
        ("for x in a; do cat <<T; done >> out\nbodyT\nT", "bodyT\n"),
        ("case x in x) cat <<T;; esac > out\nbodyT\nT", "bodyT\n"),
        ("case x in x) cat <<T;; esac >> out\nbodyT\nT", "bodyT\n"),
        // The heredoc-free twins: the `if` row is unchanged by all of this
        // (`hi` on stdout, and no `out` on `master` either); the heredoc-free
        // `for` and `case` rows *join* #298's family on task #273 — on `master`
        // they were the parse errors `expected 'done'` / `case: missing 'esac'`.
        ("if true; then echo hi; fi > out", "hi\n"),
        ("for x in a; do echo A; done > out", "A\n"),
        ("case x in x) echo C;; esac > out", "C\n"),
    ] {
        let _ = std::fs::remove_file(dir.join("out"));
        let script = format!("cd {}; {}", dir.display(), script);
        let (out, err, code) = run(&script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
        // The body went to stdout and the redirect target is never even opened.
        // bash writes the body into `out` and prints nothing.
        assert!(!dir.join("out").exists(), "{script}");
    }
}

// --- `;`-terminated heredocs in a single-line `for`/`case` body (task #273) --
//
// The same append-mode shape as the section above, for the two block builders
// that were left out of it: their bodies sit past `done`/`esac`, so each body is
// rebuilt from the regions its own `<<` claims and the closer search walks the
// block's word list as bash reads it (patterns and `for` words are word data).
// Every expectation below is bash 5.3.9's own output for the same script.

#[test]
fn single_line_for_body_heredoc_runs() {
    let (out, err, code) = run("for x in a; do cat <<T; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

#[test]
fn single_line_case_clause_body_heredoc_runs() {
    let (out, err, code) = run("case x in x) cat <<T;; esac\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

/// The matched arm of a multi-arm `case` takes its own body, in order, exactly
/// as bash's per-arm here-docs do.
#[test]
fn single_line_case_clause_body_heredocs_run_in_arm_order() {
    let script = "case a in a) cat <<A;; b) cat <<B;; esac\nbodyA\nA\nbodyB\nB";
    let (out, err, code) = run(script);
    assert_eq!((out.as_str(), code), ("bodyA\n", 0), "stderr={err:?}");
    let (out, err, code) = run(&script.replace("case a in", "case b in"));
    assert_eq!((out.as_str(), code), ("bodyB\n", 0), "stderr={err:?}");
    let (out, err, code) = run("case z in a) cat <<A;; b) cat <<B;; esac\nbodyA\nA\nbodyB\nB");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
}

/// A `for` body's two heredoc operators are claimed in order and the body runs
/// once per word, so both bodies go out on every iteration (bash's own output).
#[test]
fn single_line_for_body_two_heredocs_run_per_iteration() {
    let (out, err, code) = run("for x in a b; do cat <<A <<B; done\nbodyA\nA\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyB\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

/// The remaining single-line `for` body positions, all bash 5.3.9 parity: a
/// body that continues after the heredoc, a delimiter line that *is* the whole
/// body (an empty region), an **empty word list** (the body never runs, so the
/// region is never read — the #238 unrun-body rule), a `break` raised after the
/// body command, and a statement that follows the block on the same line.
#[test]
fn single_line_for_body_heredoc_of_a_longer_or_never_run_body_runs() {
    for (script, want) in [
        (
            "for x in a; do cat <<T; echo mid; done\nbodyT\nT",
            "bodyT\nmid\n",
        ),
        ("for x in a; do cat <<T; done\nT", ""),
        ("for x in ; do cat <<T; done\nbodyT\nT", ""),
        ("for x in a; do cat <<T; break; done\nbodyT\nT", "bodyT\n"),
        (
            "for x in a; do cat <<T; done\nbodyT\nT\necho tail",
            "bodyT\ntail\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// The body's cond list runs inside the body, both for a `for` and for a `case`
/// arm: the `&&`/`||` is the block's own cond list, and the body region travels
/// with the part that owns the operator (the #234 rule, now for #273's blocks).
#[test]
fn single_line_for_and_case_body_heredoc_runs_the_bodys_cond_list() {
    for (script, want) in [
        (
            "for q in 1; do cat <<Q; echo a && echo b; done\nbodyQ\nQ",
            "bodyQ\na\nb\n",
        ),
        (
            "case q in q) cat <<Q; echo a && echo b;; esac\nbodyQ\nQ",
            "bodyQ\na\nb\n",
        ),
        (
            "true && for q in 1; do cat <<Q; echo a || echo b; done\nbodyQ\nQ",
            "bodyQ\na\n",
        ),
        (
            "true && case q in q) cat <<Q; echo a || echo b;; esac\nbodyQ\nQ",
            "bodyQ\na\n",
        ),
        (
            "for q in 1; do echo start && cat <<Q; done\nbodyQ\nQ",
            "start\nbodyQ\n",
        ),
        (
            "case q in q) echo start && cat <<Q;; esac\nbodyQ\nQ",
            "start\nbodyQ\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// The operator *outside* the block, on a block whose bodies sit past the closer.
#[test]
fn single_line_for_and_case_body_heredoc_keeps_the_outer_cond_list() {
    for (script, want) in [
        ("true && for x in a; do cat <<T; done\nbodyT\nT", "bodyT\n"),
        ("true && case x in x) cat <<T;; esac\nbodyT\nT", "bodyT\n"),
        (
            "for x in a; do cat <<T; done && echo after\nbodyT\nT",
            "bodyT\nafter\n",
        ),
        (
            "case x in x) cat <<T;; esac && echo after\nbodyT\nT",
            "bodyT\nafter\n",
        ),
        (
            "for x in a; do cat <<T; done || echo after\nbodyT\nT",
            "bodyT\n",
        ),
        ("for x in a; do cat <<T; done | cat\nbodyT\nT", "bodyT\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err:?}");
    }
}

/// Blocks nested in a block body, each with its own body-position heredoc: the
/// closers nest, so the two bodies go out in order (bash parity).
#[test]
fn nested_single_line_block_body_heredocs_of_for_and_case_run_in_order() {
    let (out, err, code) =
        run("for x in a; do while :; do cat <<W; break; done; cat <<T; done\nbodyW\nW\nbodyT\nT");
    assert_eq!(
        (out.as_str(), code),
        ("bodyW\nbodyT\n", 0),
        "stderr={err:?}"
    );
    let (out, err, code) = run("while :; do case x in x) cat <<T;; esac; break; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

#[test]
fn single_line_for_and_case_body_heredocs_of_two_blocks_run_in_order() {
    let (out, err, code) =
        run("for x in a; do cat <<T; done; for x in a; do cat <<U; done\nbodyT\nT\nbodyU\nU");
    assert_eq!(
        (out.as_str(), code),
        ("bodyT\nbodyU\n", 0),
        "stderr={err:?}"
    );
}

/// A body line that is exactly a keyword stays body text in the `for` and
/// `case` bodies too (the #238 rule, same arm granularity).
#[test]
fn single_line_for_and_case_body_heredoc_body_keywords_are_data() {
    let (out, err, code) = run("for x in a; do cat <<T; done\ndone\nT");
    assert_eq!((out.as_str(), code), ("done\n", 0), "stderr={err:?}");
    let (out, err, code) = run("case x in x) cat <<T;; esac\nesac\nT");
    assert_eq!((out.as_str(), code), ("esac\n", 0), "stderr={err:?}");
    let (out, err, code) = run("case x in x) cat <<T;; esac\n;;\nT");
    assert_eq!((out.as_str(), code), (";;\n", 0), "stderr={err:?}");
}

/// Body lines stay opaque in a multi-line `for`/`case` too: the region is
/// scanned as data, so an operator and a keyword inside it change nothing.
#[test]
fn single_line_for_and_case_body_heredoc_body_lines_are_opaque() {
    let (out, err, code) = run("for x in a; do cat <<T; done\nbody && x\nT");
    assert_eq!((out.as_str(), code), ("body && x\n", 0), "stderr={err:?}");
    let (out, err, code) = run("for x in a; do\ncat <<T\nbodyT\nT\ndone");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
    let (out, err, code) = run("case x in x) cat <<T\nbodyT\nT\n;; esac");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

/// A body line is **data**: brace expansion never touches it, at every heredoc
/// site including a single-line keyword block's arm body (append mode), where
/// the arm's text is rebuilt from the regions its own operators claim and the
/// body bytes therefore travel *inside* that text. Task **#311** made the
/// whole-text body scan (`scan::heredoc::body_regions`) resolve a
/// `;`-terminated command line from its physical line's end, so the pre-parse
/// `brace_expand::expand` pass blanks the body there exactly as it does in a
/// newline-terminated statement (`tests/brace_expand.rs::heredoc_body_is_verbatim`).
/// Every expectation below is bash 5.3.9's own output; the body's `$`-expansion
/// neighbours (`$X` → `zzz`, `$(echo sub)` → `sub`, a literal `a,b` body →
/// `a,b`) stay in the same parity — bash's body expansion is `$`-expansion
/// only, never brace expansion.
#[test]
fn single_line_block_body_heredoc_of_a_brace_word_body_is_verbatim() {
    for (script, want) in [
        ("for x in a; do cat <<T; done\n{a,b}\nT", "{a,b}\n"),
        ("for x in a b; do cat <<T; done\n{a,b}\nT", "{a,b}\n{a,b}\n"),
        ("case x in x) cat <<T;; esac\n{a,b}\nT", "{a,b}\n"),
        ("case x in x) cat <<T; ;; esac\n{a,b}\nT", "{a,b}\n"),
        ("if true; then cat <<T; fi\n{a,b}\nT", "{a,b}\n"),
        ("if false; then :; else cat <<T; fi\n{a,b}\nT", "{a,b}\n"),
        (
            "if false; then :; elif true; then cat <<T; fi\n{a,b}\nT",
            "{a,b}\n",
        ),
        ("while :; do cat <<T; break; done\n{a,b}\nT", "{a,b}\n"),
        ("until false; do cat <<T; break; done\n{a,b}\nT", "{a,b}\n"),
        ("if true; then cat <<-T; fi\n\t{a,b}\n\tT", "{a,b}\n"),
        // Brace-shaped body bytes that are not a group stay literal too.
        (
            "if true; then cat <<T; fi\n{1..3} x{a,b}y {a,b\nT",
            "{1..3} x{a,b}y {a,b\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            (want, 0),
            "{script}: bash prints the body verbatim, rc 0; stderr={err:?}"
        );
    }
}

/// The arm's *delimiter* is protected from brace expansion at arm granularity
/// too (the block-text twin of
/// `tests/brace_expand.rs::heredoc_brace_delimiter_is_verbatim`, rc 1 on
/// `master`): the operator word and its terminating line both stay literal, so
/// the heredoc terminates and its body runs.
#[test]
fn single_line_block_heredoc_of_a_brace_word_delimiter_is_verbatim() {
    let (out, err, code) = run("if true; then cat <<{a,b}; fi\nBODY\n{a,b}");
    assert_eq!((out.as_str(), code), ("BODY\n", 0), "stderr={err:?}");
    let (out, err, code) = run("for x in a; do cat <<{a,b}; done\nBODY\n{a,b}");
    assert_eq!((out.as_str(), code), ("BODY\n", 0), "stderr={err:?}");
}

/// The same body rule covers every shape that puts a heredoc command line in a
/// block's text: the multi-line arm whose arm line ends at a `;`, a block nested
/// in a block, a pipeline and cond-list tail on the block's closer, the block
/// inside `$( … )`, a condition-position heredoc, and a block behind a
/// separator. Every expectation is bash 5.3.9's own output.
#[test]
fn block_body_heredoc_of_a_brace_word_body_is_verbatim_in_every_shape() {
    for (script, want) in [
        // The multi-line arm whose arm line ends at a `;`.
        (
            "if true; then\ncat <<T; echo hi\n{a,b}\nT\nfi",
            "{a,b}\nhi\n",
        ),
        (
            "if true; then if true; then cat <<T; fi; fi\n{a,b}\nT",
            "{a,b}\n",
        ),
        ("if true; then cat <<T; fi | cat\n{a,b}\nT", "{a,b}\n"),
        (
            "echo \"$(if true; then cat <<T; fi\n{a,b}\nT\n)\"",
            "{a,b}\n",
        ),
        (
            "case x in x) cat <<T;; esac; echo tail\n{a,b}\nT",
            "{a,b}\ntail\n",
        ),
        (
            "if true; then cat <<T; fi && echo tail\n{a,b}\nT",
            "{a,b}\ntail\n",
        ),
        (
            "echo a && if true; then cat <<T; fi\n{a,b}\nT",
            "a\n{a,b}\n",
        ),
        // A condition-position heredoc: the body is the condition's data.
        ("if cat <<T; then echo y; fi\n{a,b}\nT", "{a,b}\ny\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            (want, 0),
            "{script}: bash prints the body verbatim, rc 0; stderr={err:?}"
        );
    }
}

/// The append-mode shape reaches the text every pre-parse pass sees, so the
/// same opacity covers all of them: an unbalanced quote byte in the body no
/// longer reaches the statement tokenizer's quote check (task **#311**, the
/// body-line half of #266's class; bash prints the body, rc 0). #266 keeps the
/// **terminator-line** class — `cat <<'a <<b'` + body + `a <<b` and
/// `cat <<a\"b` + body + `a"b` stay rc 1, measured unchanged by this fix.
#[test]
fn single_line_block_body_heredoc_of_a_quoted_delimiter_and_unbalanced_body_is_verbatim() {
    for (script, want) in [
        (
            "for x in a; do cat <<'Q'; done\nbody Q unbalanced '\nQ",
            "body Q unbalanced '\n",
        ),
        (
            "case q in q) cat <<'Q';; esac\nbody Q unbalanced '\nQ",
            "body Q unbalanced '\n",
        ),
        (
            "if true; then cat <<'Q'; fi\nbody Q unbalanced '\nQ",
            "body Q unbalanced '\n",
        ),
        (
            "while :; do cat <<'Q'; break; done\nbody Q unbalanced '\nQ",
            "body Q unbalanced '\n",
        ),
        ("if true; then cat <<T; fi\n'\nT", "'\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            (want, 0),
            "{script}: bash prints the body, rc 0; stderr={err:?}"
        );
    }
}

#[test]
fn single_line_for_body_heredoc_survives_quoted_and_dash_delimiters() {
    let (out, err, code) = run("for x in a; do cat <<\"T\"; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
    let (out, err, code) = run("for x in a; do cat <<-T; done\n\tbodyT\n\tT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err:?}");
}

/// The closer search reads a `case` pattern list and a `for` word list as word
/// data (bash's rule), so a closer-shaped **pattern** never closes the block: the
/// clause keeps its body region and the block closes on its real `esac` — these
/// rows run (rc 0, the arm takes no body because `done)`/`fi)` match nothing)
/// where `master` reported `case: missing 'esac'`, rc 1. The shapes whose
/// *pattern matches* the closer-shaped word are task **#310**'s span-scan gap,
/// pinned below.
#[test]
fn closer_shaped_pattern_words_of_for_and_case_do_not_close_the_block() {
    let (out, err, code) = run("case x in done) echo hit;; esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("case x in done) cat <<T;; esac\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("case x in fi) cat <<T;; esac\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    // A `case` list that *is* a lone `esac` stays a complete block.
    let (out, err, code) = run("case x in esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
    let (out, err, code) = run("for x in a; do cat <<T; done\ndone\nT");
    assert_eq!((out.as_str(), code), ("done\n", 0), "stderr={err:?}");
}

/// Task #273's third measured divergence, pinned as fdshell's own behaviour
/// (identical on `master`): the **span** scan ends a block's text at a
/// closer-shaped word of its word list, so `done` never becomes a `for` word and
/// `done)` never becomes a clause pattern — rc 1, where bash runs both
/// (word lists are word lists). Task **#310** owns the span-level rule; when it
/// lands these rows flip to bash parity and the name drops its `310` suffix.
#[test]
fn closer_shaped_word_list_words_end_the_block_span_until_310() {
    let (_out, err, code) = run("for x in done; do echo \"$x\"; done");
    assert_eq!(code, 1, "bash prints done, rc 0");
    assert!(err.contains("expected 'do'"), "stderr={err:?}");
    let (_out, _err, code) = run("case done in done) echo p;; esac");
    assert_eq!(code, 1, "bash prints p, rc 0");
    let (_out, _err, code) = run("case done in done) cat <<T;; esac\nbodyT\nT");
    assert_eq!(code, 1, "bash prints bodyT, rc 0");
}

/// Bash applies no brace expansion to a `case` clause pattern, and fdshell
/// protects the pattern region of a clause that carries a body heredoc: the
/// arm matches the brace word and its body runs (the multi-arm twin too).
#[test]
fn case_clause_pattern_of_a_body_heredoc_is_protected_from_brace_expansion() {
    let (out, err, code) = run("case '{a,b}' in {a,b}) cat <<T;; esac\nhit\nT");
    assert_eq!((out.as_str(), code), ("hit\n", 0), "stderr={err:?}");
    let (out, err, code) = run("case '{a,b}' in {a,b}) cat <<T;; z) cat <<U;; esac\nhit\nT\nuu\nU");
    assert_eq!((out.as_str(), code), ("hit\n", 0), "stderr={err:?}");
    // A pattern that does not match takes no body, in fdshell and in bash.
    let (out, err, code) = run("case a in {a,b}) cat <<T;; esac\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err:?}");
}

/// Task #273's second measured divergence, pinned as fdshell's own behaviour:
/// a clause body terminated by `; done` closes on that word-shaped closer shape
/// and the block reports its own missing `esac` (rc 1) where bash reports a
/// syntax error (rc 2) — the rc-1-vs-2 family of
/// `midrun_adjacent_shapes_stay_measured_divergences`, same on master.
#[test]
fn case_body_heredoc_of_a_clause_terminated_by_done_reports_missing_esac() {
    let (_out, err, code) = run("case x in x) cat <<T; done;; esac\nbodyT\nT");
    assert_eq!(code, 1, "bash rc 2");
    assert!(err.contains("missing 'esac'"), "stderr={err:?}");
}

// --- the body's escape pairs and line continuations (task #269) -------------
//
// POSIX #4.2: an expandable body is a **quoted context** ("the here-document
// is equivalent to double-quoting"): `\<char>` keeps its backslash except
// `\$`, `\\`, `` \` `` and `\<newline>`, and `$( … )` / backtick / `$name`
// still expand. fdshell substituted the body with an empty quote mask, so
// every pair folded as an unquoted escape. Every expectation below is
// measured against bash 5.3.9.

#[test]
fn heredoc_body_keeps_a_non_special_escape_pair() {
    for (body, want) in [
        ("a\\ b", "a\\ b\n"),
        ("a\\*b", "a\\*b\n"),
        ("a\\nb", "a\\nb\n"),
        ("a\\\"b", "a\\\"b\n"),
        ("a\\~b", "a\\~b\n"),
    ] {
        let (out, _err, code) = run(&format!("cat <<EOF\n{body}\nEOF"));
        assert_eq!((out.as_str(), code), (want, 0), "body {body:?} vs bash");
    }
}

#[test]
fn heredoc_body_drops_the_backslash_of_its_three_special_pairs() {
    for (body, want) in [
        ("a\\$b", "a$b\n"),   // `\$`
        ("a\\\\b", "a\\b\n"), // `\\`
        ("a\\`b", "a`b\n"),   // `` \` ``
    ] {
        let (out, _err, code) = run(&format!("cat <<EOF\n{body}\nEOF"));
        assert_eq!((out.as_str(), code), (want, 0), "body {body:?} vs bash");
    }
}

#[test]
fn heredoc_body_runs_substitutions_in_a_quoted_context() {
    // `$name`, `$( … )` and backticks expand in the body exactly as bash's
    // body does, and an escaped `\$(` is protected text.
    let (out, _err, code) = run("cat <<EOF\n`echo hi` $(echo yo)\nEOF");
    assert_eq!((out.as_str(), code), ("hi yo\n", 0));
    let (out, _err, code) = run("cat <<EOF\n\\\\\\$(echo hi)\nEOF");
    // Body `\\\$(echo hi)`: the pair, then the escaped `$` — bash prints the
    // text `\$(echo hi)`.
    assert_eq!((out.as_str(), code), ("\\$(echo hi)\n", 0));
}

#[test]
fn heredoc_body_tilde_is_literal() {
    // A body is a quoted context, so a leading `~` never expands (bash prints
    // `~` and `~/x`; fdshell expanded `$HOME` with the empty mask).
    let (out, _err, code) = run("cat <<EOF\n~\n~/x\nEOF");
    assert_eq!((out.as_str(), code), ("~\n~/x\n", 0));
}

#[test]
fn heredoc_body_line_continuation_joins_the_lines() {
    let (out, _err, code) = run("cat <<EOF\na\\\nb\nEOF");
    assert_eq!((out.as_str(), code), ("ab\n", 0));
    // Consecutive continuations collapse the same way.
    let (out, _err, code) = run("cat <<EOF\na\\\n\\\nb\nEOF");
    assert_eq!((out.as_str(), code), ("ab\n", 0));
}

#[test]
fn heredoc_body_pairs_backslashes_left_to_right() {
    // `a\\` + newline: the pair is the escaped backslash, so the newline is
    // body data and the next line stays a line (bash prints `a\` and `b`).
    let (out, _err, code) = run("cat <<EOF\na\\\\\nb\nEOF");
    assert_eq!((out.as_str(), code), ("a\\\nb\n", 0));
    // `a\\\` + newline: the first two pair, the third pairs with the newline,
    // so the join leaves `a\` + `b` (bash agrees).
    let (out, _err, code) = run("cat <<EOF\na\\\\\\\nb\nEOF");
    assert_eq!((out.as_str(), code), ("a\\b\n", 0));
    // Four backslashes: pairs only, the newline stays.
    let (out, _err, code) = run("cat <<EOF\na\\\\\\\\\nb\nEOF");
    assert_eq!((out.as_str(), code), ("a\\\\\nb\n", 0));
}

#[test]
fn heredoc_quoted_delimiter_body_keeps_pairs_and_continuations() {
    // A quoted body is verbatim bytes: no pair folds and no continuation joins.
    let (out, _err, code) = run("cat <<'EOF'\na\\ b\nEOF");
    assert_eq!((out.as_str(), code), ("a\\ b\n", 0));
    let (out, _err, code) = run("cat <<'EOF'\na\\\nb\nEOF");
    assert_eq!((out.as_str(), code), ("a\\\nb\n", 0));
}

/// Accepted divergence (task #269, measured): a `<<-` body joins its
/// continuation **after** stripping the leading tabs of each physical line, so
/// the joined line's leading tab is gone. bash strips tabs from the joined
/// line, keeping it (`a<TAB>b`). rc is 0 on both sides.
#[test]
fn heredoc_dash_continuation_drops_the_joined_line_tab() {
    let (out, _err, code) = run("cat <<-EOF\n\ta\\\n\tb\nEOF");
    assert_eq!(
        (out.as_str(), code),
        ("ab\n", 0),
        "bash prints \"a\\tb\\n\""
    );
    let (out, _err, code) = run("cat <<-EOF\na\\\n\tb\nEOF");
    assert_eq!(
        (out.as_str(), code),
        ("ab\n", 0),
        "bash prints \"a\\tb\\n\""
    );
}

/// Accepted divergence (task #269, measured): a body whose last line ends with
/// `\<newline>` and whose delimiter line follows joins into the delimiter, so
/// fdshell ends the body at the delimiter line and prints the joined text
/// (`a`). bash eats the delimiter line, warns `heredoc delimited by
/// end-of-file` and prints `aEOF` — rc 0 on both sides.
#[test]
fn heredoc_body_continuation_into_the_delimiter_line_ends_the_body() {
    // The body is `a\` + newline: the continuation pair (both bytes) is the
    // body's last thing, so what is left is `a` with no newline at all.
    let (out, _err, code) = run("cat <<EOF\na\\\nEOF\n");
    assert_eq!(
        (out.as_str(), code),
        ("a", 0),
        "bash prints \"aEOF\\n\" + warning"
    );
    let (out, _err, code) = run("cat <<EOF\nab\\\nEOF\n");
    assert_eq!(
        (out.as_str(), code),
        ("ab", 0),
        "bash prints \"abEOF\\n\" + warning"
    );
}

/// A body that ends with a trailing `\<newline>` and has **no** delimiter line
/// is fdshell's usual unterminated-heredoc error, the same rc 1 as the plain
/// `cat <<EOF` + body. bash only warns (rc 0).
#[test]
fn heredoc_unterminated_body_with_continuation_is_a_parse_error() {
    let (_out, err, code) = run("cat <<EOF\nab\\\n");
    assert_eq!(code, 1);
    assert!(err.contains("here-doc"), "stderr={err:?}");
    assert!(err.contains("missing terminating"), "stderr={err:?}");
}

/// Accepted divergence (task #222, to be closed by task #288): a here-doc body
/// is the substituted text and is never split, so a **quoted** `$@` keeps task
/// #222's separator-free join and the positionals glue, where bash joins them
/// with a space. A quoted delimiter is literal, so `$@` there expands to
/// nothing in both shells.
#[test]
fn quoted_dollar_at_in_a_heredoc_body_is_glued() {
    // bash prints `a b c`.
    let (out, err, code) = run("set -- \"a b\" c; cat <<EOF\n$@\nEOF");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "a bc\n", "bash prints a b c");
    // Guard: a quoted delimiter expands nothing — both print the text `$@`.
    let (out, _err, code) = run("set -- \"a b\" c; cat <<'EOF'\n$@\nEOF");
    assert_eq!(code, 0);
    assert_eq!(out, "$@\n");
}

/// Task #232: a here-doc in the body of a **mid-run** block (a block that
/// opens after `&&` / `||`) claims its body region through the multi-line
/// run: `cat` prints the body, the loop continues, and a `&&` written in
/// the body line never splits the part (measured against bash).
#[test]
fn midrun_block_body_heredoc_is_region_scanned() {
    let (out, err, code) = run("true && while :; do cat <<T; echo mid; break; done\nbody\nT");
    assert_eq!((out.as_str(), code), ("body\nmid\n", 0), "stderr={err}");
    let (out, err, code) = run("cat <<X && echo b\nq && r\nX");
    assert_eq!((out.as_str(), code), ("q && r\nb\n", 0), "stderr={err}");
}

// ── The io-number form: `[N]<<word` (task #276) ──────────────────────────────
// Every expectation below was measured against bash 5.3.9 (`bash -c`) and
// `target/debug/fdshell`; the rows named "divergence" are documented in
// POSIX_COMPATIBILITY.md and reference the task that owns the gap.

#[test]
fn numbered_heredoc_delivers_body_to_the_descriptor() {
    // bash: `hello`, rc 0 — the body is fd 3's input, stdin stays untouched.
    let (out, _err, code) = run("exec 3<<T\nhello\nT\ncat <&3");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn numbered_heredoc_zero_target_feeds_stdin() {
    let (out, _err, code) = run("cat 0<<T\nbody\nT");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n");
}

#[test]
fn numbered_heredoc_leaves_stdin_untouched() {
    let (out, _err, code) = run("cat 3<<T\nbody\nT");
    assert_eq!(code, 0);
    assert_eq!(out, "");
}

#[test]
fn numbered_heredoc_quoted_delimiter_is_literal() {
    let (out, _err, code) = run("X=zzz; exec 3<<'Q'\n$X\nQ\ncat <&3");
    assert_eq!(code, 0);
    assert_eq!(out, "$X\n");
}

#[test]
fn numbered_heredoc_dash_form_strips_leading_tabs() {
    let (out, _err, code) = run("exec 3<<-T\n\thello\n\tT\ncat <&3");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn numbered_heredoc_bare_form_takes_the_next_word() {
    // `3<< T`: the delimiter is the next word, so the body starts on the
    // following line and the operator line's tail is not its body.
    let (out, _err, code) = run("exec 3<< T\nbare\nT\ncat <&3");
    assert_eq!(code, 0);
    assert_eq!(out, "bare\n");
}

#[test]
fn numbered_heredoc_targets_are_independent() {
    let (out, _err, code) = run("exec 3<<T\nfirst\nT\nexec 4<<U\nsecond\nU\ncat <&3\ncat <&4");
    assert_eq!(code, 0);
    assert_eq!(out, "first\nsecond\n");
}

#[test]
fn numbered_heredoc_dup_of_the_target_keeps_the_body() {
    // The body's source descriptor is allocated above every export target, so
    // `3<&3` (a dup onto the target itself) cannot truncate what fd 3 holds.
    let (out, _err, code) = run("exec 3<<T\nhello\nT\nexec 3<&3\ncat <&3");
    assert_eq!(code, 0);
    assert_eq!(out, "hello\n");
}

#[test]
fn numbered_heredoc_in_a_block_targets_the_descriptor() {
    let (out, _err, code) = run("if true; then exec 3<<T; fi\nbody\nT\ncat <&3");
    assert_eq!(code, 0);
    assert_eq!(out, "body\n");
}

/// The io-number form composes with the multi-line positions the region
/// machinery of #138/#238 established: a single-line block body, a block
/// **condition**, and the `for`/`case` bodies. Each body line there is a
/// command name, so `rc == 0` is the assertion that the line's operator
/// claimed its body region and the body never ran as statements; the `exec`
/// twins show the body lands on fd 3 in the same positions. Measured against
/// bash 5.3.9 (`if` condition row: `t`; the rest: empty stdout).
#[test]
fn numbered_heredoc_composes_with_the_block_condition_and_loop_positions() {
    for (script, want) in [
        ("if true; then cat 3<<T; fi\nnotacmd\nT", ""),
        ("if cat 3<<T; then echo t; fi\nnotacmd\nT", "t\n"),
        ("for x in a; do cat 3<<T; done\nnotacmd\nT", ""),
        ("case a in a) cat 3<<T;; esac\nnotacmd\nT", ""),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            (want, 0),
            "script={script:?} err={err}"
        );
    }
    for script in [
        "for x in a; do exec 3<<T; done\nbody\nT\ncat <&3",
        "case a in a) exec 3<<T;; esac\nbody\nT\ncat <&3",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            ("body\n", 0),
            "script={script:?} err={err}"
        );
    }
}

#[test]
fn numbered_heredoc_is_not_a_path_open() {
    // The old defect opened the path `<T` (`failed to open redirection path`);
    // the `<<` operator never reaches the path rule.
    let (out, err, code) = run("echo a 3<<T\nbody\nT");
    assert_eq!(code, 0);
    assert_eq!(out, "a\n");
    assert!(!err.contains("redirection path"), "{err}");
}

#[test]
fn numbered_heredoc_word_is_not_an_argument() {
    // `3<<T` is an operator, not a word: only `x` reaches the child.
    let (out, _err, code) = run("echo x 3<<T\nbody\nT");
    assert_eq!(code, 0);
    assert_eq!(out, "x\n");
}

#[test]
fn numbered_heredoc_body_lines_are_not_executed() {
    let (out, _err, code) = run("cat 3<<T\nbody\nT\necho after");
    assert_eq!(code, 0);
    assert_eq!(out, "after\n");
}

#[test]
fn numbered_heredoc_in_a_pipeline_and_cond_list() {
    let (out, _err, code) = run("cat 3<<T | wc -c\nbody\nT");
    assert_eq!(code, 0);
    assert_eq!(out, "0\n");
    let (out, _err, code) = run("cat 3<<T && echo yes\nbody\nT");
    assert_eq!(code, 0);
    assert_eq!(out, "yes\n");
}

/// A stage's share of the body regions is `count` long and the walk advances
/// by that count, so a stage with **two** operators leaves the next stage's
/// own body: `wc -c` counts `ccc\n` = 4, bash-identical (a walk that multiplies
/// instead of adds hands stage 2 `aaaa\n` = 5; the unit twin is
/// `test_heredoc_pipeline_spec_window_advances_by_operator_count`).
#[test]
fn heredoc_pipeline_spec_window_survives_a_two_operator_stage() {
    let (out, _err, code) = run("cat <<A <<B | wc -c <<C\naaaa\nA\nbb\nB\nccc\nC");
    assert_eq!(code, 0);
    assert_eq!(out, "4\n");
}

/// A `#` comment at word-start index 1 on the run's first **line**, its
/// newline the very next byte (`i = 1`, `p = 1`): the skip lands on
/// `i + p + 1 = 3`, the first byte of the next line, and that line's operator
/// is still found — prints `b`, rc 0, bash-identical. This row pins the
/// comment skip at `i > 0, p > 0`, the shape the index-0 rows do not cover; it
/// kills no comment mutant (`p >= 1` always, so the multiplied `i * p + 1`
/// lands on the newline at 2 and the extra iteration consumes it
/// state-neutrally — LESSONS; the mutants are killed by the index-0 rows).
/// Unit twin
/// `comment_at_index_one_with_next_line_newline_resumes_the_scan`.
#[test]
fn comment_line_at_a_run_start_leaves_the_next_line_operator() {
    let (out, _err, code) = run(" #\ncat <<A\nb\nA");
    assert_eq!(code, 0);
    assert_eq!(out, "b\n");
}

/// A digit run *inside a quoted word* is word data: `cat x" 3<<T"` passes the
/// whole word to `cat` (rc 1) and the body lines run as statements. The
/// quoted span has a space at index 4 — a raw-byte word start **inside** the
/// token — and the token-level rule must reject a prefix that does not begin
/// the span, or the token count claims an operator the byte scan never sees
/// (the count-agreement rule, LESSONS).
#[test]
fn numbered_heredoc_shape_inside_a_quoted_word_is_word_data() {
    for script in [
        "cat x\" 3<<T\"\nbody\nT",
        "cat \" 3<<T\"\nbody\nT",
        "cat x' 3<<T'\nbody\nT",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "script={script:?} err={err}");
        assert_eq!(out, "", "script={script:?}");
        // The whole word reaches `cat`; no here-doc, no fd-3 operator.
        assert!(err.contains("No such file"), "script={script:?} err={err}");
        assert!(!err.contains("valid descriptor number"), "{err}");
        // The body lines are statements, not a consumed here-doc body.
        assert!(
            err.contains("\"body\" not found"),
            "script={script:?} err={err}"
        );
    }
}

/// The `N<<word` rule reads raw bytes, so a prefix that is quoted, escaped or
/// glued to a longer word is word data: the token reaches the child as one
/// argument and there is no here-doc. bash agrees (its io-number must be an
/// unquoted word of digits); only the exit status of the follow-on commands
/// differs (fdshell reports 1, bash 127 — pinned in tests/external.rs).
/// The `cat 3<<<str` row that used to sit in this list is gone: task #312 made
/// it a here-string on fd 3, and the here-string family pins that shape (and
/// its own word-data rows) in `tests/herestring.rs`.
#[test]
fn numbered_heredoc_prefixes_that_are_not_digits_are_word_data() {
    for (script, needle) in [
        ("cat x<<T\nbody\nT", "cat: 'x<<T'"),
        ("cat x3<<T\nbody\nT", "cat: 'x3<<T'"),
        ("cat -3<<T\nbody\nT", "cat: invalid option"),
        ("cat \"3\"<<T\nbody\nT", "cat: '3<<T'"),
        ("cat 3\"<<\"T\nbody\nT", "cat: '3<<T'"),
        ("cat 3'<<'T\nbody\nT", "cat: '3<<T'"),
        ("cat \\3<<T\nbody\nT", "cat: '3<<T'"),
        ("FD=3; cat $FD<<T\nbody\nT", "cat: '3<<T'"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "script={script:?} err={err}");
        assert_eq!(out, "", "script={script:?}");
        // The word reaches the child as an argument: no here-doc, no path
        // open of `<T`, no descriptor-number report.
        assert!(err.contains(needle), "script={script:?} err={err}");
        assert!(!err.contains("redirection path"), "script={script:?}");
        assert!(
            !err.contains("valid descriptor number"),
            "script={script:?}"
        );
    }
}

/// A digit run that is not a descriptor number is a parse error naming the
/// prefix — fdshell's io-number rule is the `N>file` fd rule, where bash
/// treats the whole word as a command argument (`cat: 999…: No such file`).
#[test]
fn numbered_heredoc_rejects_an_io_number_that_is_not_a_descriptor() {
    let (out, err, code) = run("cat 99999999999999999999999<<T\nb\nT");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("is not a valid descriptor number"), "{err}");
}

/// The largest `i32` fd number is accepted by the rule and rejected when the
/// descriptor is opened (`cat: 2147483647: Bad file descriptor` in bash).
#[test]
fn numbered_heredoc_largest_io_number_is_reported_at_open() {
    let (out, err, code) = run("cat 2147483647<<T\nb\nT");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(
        err.contains("file descriptor number is out of range"),
        "{err}"
    );
}

/// Divergences pinned as measured, each owned by the task named in its comment.
#[test]
fn numbered_heredoc_known_divergences() {
    // A `<<` at the run start is fdshell's command word (the operator rule needs
    // a preceding token), so `3<<T` alone runs as a command: rc 1. bash reads it
    // as a here-doc on fd 3 with no command: rc 0.
    let (out, _err, code) = run("3<<T\nbody\nT");
    assert_eq!(code, 1);
    assert_eq!(out, "");

    // A sibling `>&N` dup takes its source from the parent's fd table, so the
    // descriptor that `3<<T` opens in the same command is invisible to it
    // (task #178, order of redirections). Run through `run_fd3_closed`: the
    // assertion is that fd 3 is closed *in the parent*, which only holds on a
    // deterministic fd table (an inherited fd 3 — cargo-mutants' jobserver —
    // would make `1>&3` succeed).
    let (out, err, code) = run_fd3_closed("cat 3<<T 1>&3\nbody\nT");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("fd 3 is not open"), "{err}");

    // A here-doc in a single-line group body claims no body region (task #315);
    // the plain form fails identically.
    let (out, err, code) = run("{ cat 3<<T; }\nbody\nT");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(
        err.contains("missing terminating delimiter line 'T'"),
        "{err}"
    );

    // `( … )` is not a subshell: `(` reaches the command lookup (task #316).
    let (out, err, code) = run("( cat 3<<T )\nbody\nT");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("\"(\" not found"), "{err}");
}

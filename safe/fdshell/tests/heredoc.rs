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

/// POSIX XCU 2.3 quotes the delimiter word on **any** quote operator, and `\`
/// is one: a word with no `"`/`'` at all is quoted by its escape pairs alone,
/// so `<<\E\O\F` delimits the folded `EOF` with a literal body.
#[test]
fn heredoc_backslash_delimiter_is_literal() {
    for (script, want) in [
        ("X=zzz; cat <<\\E\\O\\F\n$X\nEOF", "$X\n"),
        ("X=zzz; cat <<\\EOF\n$X\nEOF", "$X\n"),
        ("X=zzz; cat <<\\$HOME\n$(echo hi)\n$HOME", "$(echo hi)\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "stderr={err:?} script={script:?}");
        assert_eq!(
            out, *want,
            "the word is quoted by its pairs: script={script:?}"
        );
    }
}

/// Every position of the pair quotes the word: `<<E\OF`, `<<EO\F`, `<<\EOF`.
#[test]
fn heredoc_escaped_delimiter_is_literal() {
    for script in [
        "X=zzz; cat <<E\\OF\n$X\nEOF",
        "X=zzz; cat <<EO\\F\n$X\nEOF",
        "X=zzz; cat <<\\EOF\n$X\nEOF",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "stderr={err:?} script={script:?}");
        assert_eq!(out, "$X\n", "script={script:?}");
    }
}

/// `<<\\EOF` folds to the delimiter `\EOF` (the pair `\\` is the fold, the
/// folded pair is the quoting), so the terminator line is the literal `\EOF`
/// and the body is literal.
#[test]
fn heredoc_escaped_pair_delimiter_matches_its_folded_terminator() {
    let (out, err, code) = run("X=zzz; cat <<\\\\EOF\n$X\n\\EOF");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "$X\n");
}

/// An escaped quote is data in the delimiter text (`<<a\'b\'c` delimits
/// `a'b'c`, a balanced terminator), and the pairs quote the body (task #263).
#[test]
fn heredoc_escaped_quote_delimiter_is_literal() {
    let (out, err, code) = run("X=zzz; cat <<a\\'b\\'c\n$X\na'b'c");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "$X\n");
}

/// The `<<-` form keeps stripping tabs with an escape-quoted delimiter.
#[test]
fn heredoc_dash_escaped_delimiter_is_literal_and_stripped() {
    let (out, err, code) = run("X=zzz; cat <<-E\\OF\n\t$X\n\tb\n\tEOF");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "$X\nb\n");
}

/// The bare form inherits the marker and quotes the body the same way.
#[test]
fn heredoc_bare_escaped_delimiter_is_literal() {
    for script in [
        "X=zzz; cat <<- E\\OF\n\t$X\n\tEOF",
        "X=zzz; cat << E\\OF\n$X\nEOF",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "stderr={err:?} script={script:?}");
        assert_eq!(out, "$X\n", "script={script:?}");
    }
}

/// A quoted body is verbatim on both counts: no `$`/`$( … )`/backtick
/// expansion, and no `\<newline>` continuation stripped (same `quoted` gate).
#[test]
fn heredoc_escaped_delimiter_body_keeps_substitutions_and_continuations() {
    for (script, want) in [
        ("X=zzz; cat <<E\\OF\n$(echo hi)\nEOF", "$(echo hi)\n"),
        ("X=zzz; cat <<E\\OF\n`echo hi`\nEOF", "`echo hi`\n"),
        ("X=zzz; cat <<E\\OF\n\\$X\nEOF", "\\$X\n"),
        ("X=zzz; cat <<E\\OF\na\\\nb\nEOF", "a\\\nb\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 0, "stderr={err:?} script={script:?}");
        assert_eq!(out, *want, "script={script:?}");
    }
}

/// Two escape-quoted heredocs on one command line: bash's fd 0 ends up the
/// **last** body, literal (`cat <<A\X <<B\Y` — the second body follows `AX`).
#[test]
fn heredoc_two_escaped_delimiters_on_one_line_are_literal() {
    let (out, err, code) = run("X=zzz; cat <<A\\X <<B\\Y\n$X\nAX\n$X\nBY");
    assert_eq!(code, 0, "stderr={err:?}");
    assert_eq!(out, "$X\n");
}

/// A block arm's heredoc: the command line ends at `fi`, the body follows the
/// block line, and the escape-quoted delimiter keeps it literal.
#[test]
fn heredoc_block_arm_escaped_delimiter_is_literal() {
    let (out, err, code) = run("X=zzz; if true; then cat <<E\\OF; fi\n$X\nEOF");
    assert_eq!(code, 0, "stderr={err:?}");
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

// --- Function-definition blocks on a line that also carries heredoc statements
// (task #240): the definition claims **no** region of its physical line's body
// stream (its body's here-docs feed the statements inside it, the body is
// re-scanned), and the regions are allocated per physical line in operator
// order: the pre-block statements first, the trailing statements last. On
// master the trailing statement was fed the pre-block statement's body region
// (`bodyX` plus its delimiter line as content). Every expectation below is
// bash 5.3.9's own output for the same script. ---

#[test]
fn heredoc_stmt_before_function_block_and_after_same_line() {
    let (out, err, code) = run("cat <<X; f() { echo hi; }; f; cat <<B\nbodyX\nX\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nhi\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_two_stmts_before_function_block_and_after_same_line() {
    let (out, err, code) =
        run("cat <<X; cat <<Y; f() { echo hi; }; f; cat <<B\nbodyX\nX\nbodyY\nY\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nbodyY\nhi\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_stmt_before_function_block_ending_line_same_line() {
    // The definition ends the line: the scan resumes past all of the line's
    // body regions, so `bodyX` is data, never a command (master: rc 1).
    let (out, err, code) = run("cat <<X; f() { echo hi; }\nbodyX\nX");
    assert_eq!((out.as_str(), code), ("bodyX\n", 0), "stderr={err:?}");
}

#[test]
fn heredoc_stmt_before_function_keyword_form_and_after_same_line() {
    let (out, err, code) = run("cat <<X; function f { echo hi; }; f; cat <<B\nbodyX\nX\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nhi\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_stmt_before_function_block_two_calls_and_after_same_line() {
    let (out, err, code) = run("cat <<X; f() { echo hi; }; f; f; cat <<B\nbodyX\nX\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nhi\nhi\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_two_function_defs_redefine_and_trailing_stmt_same_line() {
    let (out, err, code) =
        run("cat <<X; f() { echo hi; }; f() { echo second; }; f; cat <<B\nbodyX\nX\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nsecond\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_stmt_between_function_block_and_call_same_line() {
    let (out, err, code) = run("cat <<X; f() { echo hi; }; cat <<B; f\nbodyX\nX\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nbodyB\nhi\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_stmt_before_function_block_and_conditional_tail_same_line() {
    let (out, err, code) = run("cat <<X; f() { echo hi; }; f && cat <<B\nbodyX\nX\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nhi\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_stmt_before_function_block_and_redirected_call_same_line() {
    let (out, err, code) = run("cat <<X; f() { :; }; f > /dev/null; cat <<B\nbodyX\nX\nbodyB\nB");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_stmt_before_function_block_nested_in_if_body_same_line() {
    // The same rule inside a keyword block's body (task #242 owns the
    // nested-combo xtrace shape, not these outputs).
    let (out, err, code) =
        run("if true; then cat <<X; f() { echo hi; }; f; cat <<B\nbodyX\nX\nbodyB\nB\nfi");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nhi\nbodyB\n", 0),
        "stderr={err:?}"
    );
}

#[test]
fn heredoc_in_multiline_function_then_trailing_stmt_same_line() {
    // Multi-line definition: the trailing statement keeps master's (already
    // bash-matching) region — regression guard for the multi-line branch.
    let (out, err, code) = run("f() {\ncat <<A\nin-a\nA\n}\nf; cat <<B\nbodyB\nB");
    assert_eq!((out.as_str(), code), ("in-a\nbodyB\n", 0), "stderr={err:?}");
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
/// "Errors"), and the `break` that ends the loop leaves rc 0 — bash's, measured
/// 2026-10-19; the gap this pin used to assert (rc 1) was task #262's, now
/// fixed. stdout is the block's heredoc plus nothing else.
#[test]
fn block_with_heredoc_runs_a_failing_body_cond_list() {
    let (out, _err, code) = run("while cat <<X; do a && b; break; done\nbody\nX");
    assert_eq!((out.as_str(), code), ("body\n", 0));
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

/// Task #304 (the consequence task #234's review filed): the region rule scans a
/// heredoc **body region** for a top-level `&&` / `||` only where the `<<` that
/// claimed it sits in the block's **clause** text. A region claimed by a word of
/// the block's **body** is the body's own data, exactly as the same line outside
/// any heredoc: bash prints it, rc 0. This is the shape whose pin carried the
/// `…_until_304` name until #304's position gate landed. Measured 2026-10-23
/// against bash 5.3.9(1)-release (rc 0, body printed): master 1adbef0 has parity
/// **by accident** (its `block_rule` gate turned the block counter off for any
/// heredoc-carrying text), task #234's tree lost it (rc 1 `missing 'fi'` /
/// `case: missing 'esac'` / `expected 'done'`), and #304 restores the parity
/// while keeping the clause-position split pinned by
/// [`block_heredoc_cond_run_and_and_in_body_splits_part`].
#[test]
fn and_in_a_body_position_heredoc_region_is_body_data() {
    // The `if` body, `&&` in the body line: the body prints, rc 0.
    let (out, err, code) = run("if :; then cat <<Q\nz && y\nQ\nfi");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
    // With a tail operator behind the closer: the block runs, the tail runs.
    let (out, err, code) = run("if :; then cat <<Q\nz && y\nQ\nfi && echo tail");
    assert_eq!((out.as_str(), code), ("z && y\ntail\n", 0), "stderr={err}");
    // A quoted delimiter, `||` in the body, tail operator: bash's parity row.
    let (out, err, code) = run("if :; then cat <<'Q'\nz || y\nQ\nfi && echo tail");
    assert_eq!((out.as_str(), code), ("z || y\ntail\n", 0), "stderr={err}");
    // The `case` twin (bounded arm): bash prints the body, rc 0.
    let (out, err, code) = run("case q in q) cat <<'Q'\nz && y\nQ\n;;\nesac");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
    // The bounded `while` twin: master looped (bash-identical, rc 124 under a
    // timeout); with `break` the loop body's operator is body data and the tail
    // runs, rc 0 — bash's.
    let (out, err, code) = run("while :; do cat <<Q\nz && y\nQ\nbreak\ndone && echo tail");
    assert_eq!((out.as_str(), code), ("z && y\ntail\n", 0), "stderr={err}");
}

/// Task #304: the body-position rule covers every block shape — a `for` and an
/// `until` body, an `else` arm, a nested block, a group written in a body, a
/// run that enters the block behind a `&&`, a `case` arm's nested block, and a
/// two-operator line whose **both** regions are body data. Measured 2026-10-23
/// against bash 5.3.9(1)-release; every row is rc 1 (`missing 'fi'` /
/// `expected 'done'` / `case: missing 'esac'`) on master and rc 0 here, with the
/// stdout bash prints.
#[test]
fn a_body_position_heredoc_region_in_every_block_shape_prints_its_body() {
    // A `for` body (bounded by the one-item word list).
    let (out, err, code) = run("for x in a; do cat <<Q\nz && y\nQ\ndone");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
    // An `until` body that never runs: bash's rc 0 with no output at all.
    let (out, err, code) = run("until :; do cat <<Q\nz && y\nQ\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    // An `else` arm's body (never taken): rc 0, no output.
    let (out, err, code) = run("if :; then :; else cat <<Q\nz && y\nQ\nfi");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    // A nested block: the innermost block's mode is the body.
    let (out, err, code) = run("if :; then if :; then cat <<Q\nz && y\nQ\nfi\nfi");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
    // A `{ …; }` group written in a block body keeps the body mode.
    let (out, err, code) = run("if :; then { cat <<Q\nz && y\nQ\n} fi");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
    // A group wrapping the block.
    let (out, err, code) = run("{ if :; then cat <<Q\nz && y\nQ\nfi\n}");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
    // One line, two operators: **both** regions are body data.
    let (out, err, code) = run("if :; then cat <<Q; cat <<R\nz && y\nQ\nu && v\nR\nfi");
    assert_eq!(
        (out.as_str(), code),
        ("z && y\nu && v\n", 0),
        "stderr={err}"
    );
    // The block entered behind a `&&`, and a statement behind its closer.
    let (out, err, code) = run("echo pre && if :; then cat <<Q\nz && y\nQ\nfi");
    assert_eq!((out.as_str(), code), ("pre\nz && y\n", 0), "stderr={err}");
    let (out, err, code) = run("if :; then cat <<Q\nz && y\nQ\nfi\necho after");
    assert_eq!((out.as_str(), code), ("z && y\nafter\n", 0), "stderr={err}");
    // The body's own lines after the region keep splitting (the block's text is
    // opaque, the region is data, and the block's `echo a && echo b` is the
    // block's own cond list).
    let (out, err, code) = run("if :; then\ncat <<Q\nz && y\nQ\necho a && echo b\nfi");
    assert_eq!((out.as_str(), code), ("z && y\na\nb\n", 0), "stderr={err}");
    // A body heredoc in a nested block of a loop body / a `case` arm, bounded.
    let (out, err, code) = run("while :; do\nif :; then cat <<Q\nz && y\nQ\nfi\nbreak\ndone");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
    let (out, err, code) = run("case q in\nq)\nif :; then cat <<Q\nz && y\nQ\nfi\n;;\nesac");
    assert_eq!((out.as_str(), code), ("z && y\n", 0), "stderr={err}");
}

/// Task #304: the **clause**-position family does **not** move. A `<<` written in
/// a block's condition (the condition, the word list, an `elif` condition, a
/// group in a condition) travels inside the block's span, its region is scanned,
/// and the split ends where bash also rejects the text — bash rc 2, fdshell rc 1
/// (the accepted rc-1-vs-2 family the README names). Measured 2026-10-23 against
/// bash 5.3.9(1)-release; identical on master 1adbef0 and on #304.
#[test]
fn a_clause_position_region_still_splits_the_block_text() {
    // The pinned clause split (`block_heredoc_cond_run_and_and_in_body_splits_part`),
    // the `elif` condition, a group in a condition, a condition with two
    // operators, and a nested block's condition.
    for script in [
        "if cat <<X; then\nbody && x\nX\nfi",
        "if :; then :; elif cat <<Q; then\nz && y\nQ\nfi",
        "if { cat <<Q; } then\nz && y\nQ\nfi",
        "if cat <<Q; cat <<R; then\nz && y\nQ\nu && v\nR\nfi",
        "if :; then\nif cat <<X; then\nb && x\nX\nfi\nfi",
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "bash rc 2 for {script:?}");
        assert!(err.contains("parse error"), "stderr={err} for {script:?}");
    }
}

/// Task #304, the divergence its plan predicted and its sweep measured: a word
/// that **ends** with `)` sets the innermost block's mode to body (the row is
/// `keywords/position::after`'s own, ungated, and the mode stack mirrors it), so
/// in a block's *clause* a following heredoc region stops splitting and fdshell
/// runs text bash rejects: bash rc 2, master rc 1, here rc 0. Task **#430** owns
/// the block-kind knowledge that separates a `case` arm's `)` from a subshell
/// `)`; the mode stack keeps the ungated row until then.
#[test]
fn a_word_ending_in_close_paren_in_a_clause_stops_the_split() {
    // bash: `syntax error near unexpected token 'fi'` / `')'`; master: rc 1
    // `missing 'fi'` for all three (the region split); here: rc 0.
    let (out, err, code) = run("if (cat) && cat <<X; then\nz && y\nX\nfi");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    assert!(
        err.contains("failed to resolve command path"),
        "stderr={err}"
    );
    let (out, err, code) = run("if (echo a); then cat <<X; then\nz && y\nX\nfi");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    let (out, err, code) = run("if :; then echo a); cat <<X\nz && y\nX\nfi");
    assert_eq!((out.as_str(), code), ("a )\nz && y\n", 0), "stderr={err}");
    // The heredoc-free twins are the *pre-existing* half of the shape (fdshell
    // accepts the second `then`; master and here agree): the divergence this pin
    // records is the region-scan class, not the acceptance.
    let (_out, _err, code) = run("if (echo a); then echo t; then echo u; fi");
    assert_eq!(code, 0, "bash rc 2, accepted on master and here alike");
}

/// Task #304: the event-case `wait` block's arm pattern ends with `)` too, so its
/// arm's heredoc region is body data and the block reaches the parser. bash has
/// no `wait …)` grammar at all (rc 2); master reported the *parse* failure
/// `expected 'done'` (the region split the block in half), here the block parses
/// and the run-level report is the shell's own (the `%s` fd variable is unset).
/// Measured 2026-10-23; both rc 1.
#[test]
fn a_wait_block_arm_heredoc_region_is_a_body_region() {
    let (_out, err, code) = run("wait readable %s) cat <<Q\nz && y\nQ\n;;\ndone");
    assert_eq!(code, 1, "bash rc 2 (no such grammar)");
    assert!(err.contains("s is not set"), "stderr={err}");
}

/// Task #304, the `# c` twin of the body-position pin, re-pinned as measured:
/// the region-rule half is closed (no `missing 'fi'` any more — the body-position
/// region is opaque), and what is left is the **statement-level** divergence of a
/// `#` comment written behind a here-doc delimiter word, which reproduces with no
/// block at all. bash prints the body at rc 0; master and here are rc 1
/// (`here-doc: missing terminating delimiter line`). Task **#429** owns that
/// family; the same row is pinned in
/// `tests/cond_blocks.rs::a_comment_at_a_block_edge_keeps_the_filed_divergences`.
#[test]
fn a_comment_behind_a_heredoc_line_is_the_statement_level_divergence() {
    let (_out, err, code) = run("if :; then cat <<Q # c\nq && y\nQ\nfi");
    assert_eq!(code, 1, "bash rc 0 prints 'q && y'");
    assert!(
        err.contains("missing terminating delimiter line 'Q'"),
        "stderr={err}"
    );
    // The statement twin, with no block at all: the same rc and report on master
    // and here, so the block is not what breaks this shape.
    let (_out, err, code) = run("cat <<Q # c\nq && y\nQ");
    assert_eq!(code, 1, "bash rc 0 prints 'q && y'");
    assert!(
        err.contains("missing terminating delimiter line 'Q'"),
        "stderr={err}"
    );
}

/// Task #275 (measured 2026-10-21, bash 5.3.9): the line's regions are allocated
/// **greedily, one entry per operator**, so an unterminated `<<E` in the `elif`
/// condition claims the (empty) rest of the stream and the `then` arm keeps its
/// own complete body: `bodyT`, rc 0. bash prints the same and additionally warns
/// `here-document at line 3 delimited by end-of-file (wanted 'E')` — the warning
/// is task **#364**'s.
#[test]
fn single_line_block_body_heredoc_of_an_incomplete_line_stream_prints_the_complete_arm() {
    let (out, _err, code) = run("if true; then cat <<T; elif cat <<E; then echo b; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0));
}

/// Task #275, the statement twins of the acceptance shape (measured 2026-10-21,
/// bash 5.3.9 — stdout and rc identical, bash adds the end-of-file warning): the
/// statement whose delimiter line arrived keeps its body, and the unterminated
/// operator behind it takes what is left of the stream as its own body.
#[test]
fn an_unterminated_heredoc_behind_a_complete_one_keeps_the_complete_body() {
    let (out, _err, code) = run("cat <<T; cat <<E\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "bash: bodyT, rc 0");
    // The rest-of-stream claim: the lines left after the complete body are the
    // unterminated operator's **body**, never commands (`extra` prints, rc 0).
    let (out, _err, code) = run("cat <<T; cat <<E\nbodyT\nT\nextra");
    assert_eq!(
        (out.as_str(), code),
        ("bodyT\nextra\n", 0),
        "bash: bodyT extra, rc 0"
    );
    let (out, _err, code) = run("cat <<T; cat <<E\nbodyT\nT\nextra\nmore");
    assert_eq!(
        (out.as_str(), code),
        ("bodyT\nextra\nmore\n", 0),
        "bash: bodyT extra more, rc 0"
    );
}

/// Task #275, the condition position (measured 2026-10-21: bash prints `bodyT`
/// and `y`, rc 0): a complete condition heredoc and a starved arm heredoc on the
/// same line — the condition claims its region, the arm reads an empty body.
#[test]
fn a_complete_block_condition_heredoc_survives_an_unterminated_arm_one() {
    let (out, _err, code) = run("if cat <<T; then echo y; cat <<E; fi\nbodyT\nT");
    assert_eq!(
        (out.as_str(), code),
        ("bodyT\ny\n", 0),
        "bash: bodyT y, rc 0"
    );
}

/// Task #275, the arm-shape twins (measured 2026-10-21: `bodyT`, rc 0 in bash):
/// a `case` clause, a `for` arm and a `while` arm with one complete and one
/// unterminated operator on the same line.
#[test]
fn an_unterminated_heredoc_behind_a_complete_one_in_a_case_clause_and_a_loop_arm() {
    let (out, _err, code) = run("case x in x) cat <<T; cat <<E;; esac\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "bash: bodyT, rc 0");
    let (out, _err, code) = run("for i in 1; do cat <<T; cat <<E; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "bash: bodyT, rc 0");
    let (out, _err, code) = run("while :; do cat <<T; cat <<E; break; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "bash: bodyT, rc 0");
}

/// Task #275, the `<<-` form (measured 2026-10-21: `bodyT`, rc 0 in bash): the
/// synthesized terminator line of the starved operator is tab-stripped exactly
/// like a real one, because it is written untabbed.
#[test]
fn an_unterminated_dashed_heredoc_behind_a_complete_one_keeps_the_complete_arm() {
    let (out, _err, code) = run("if true; then cat <<-T; elif cat <<-E; then echo b; fi\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "bash: bodyT, rc 0");
}

/// Task #275, a block with a trailing statement on its line (measured
/// 2026-10-21: bash `bodyT`, rc 0 + warning): the tail claim consumes the rest
/// of the stream, so the scan resumes at the text end and the trailing
/// `cat <<E` runs with an empty body, printing nothing.
#[test]
fn a_tail_claim_past_a_block_leaves_the_trailing_statement_an_empty_body() {
    let (out, _err, code) = run("if true; then cat <<T; fi; cat <<E\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "bash: bodyT, rc 0");
}

/// Task #275's **residual**, pinned as measured: a line whose **first** `<<`
/// operator has no delimiter line stays fdshell's parse error — the greedy
/// allocation claims nothing when `found == 0` and the parse layer owns "this
/// heredoc has no body at all". bash warns and runs (rc 0, its body is the
/// remaining lines); task **#364** owns that whole end-of-file family.
#[test]
fn a_line_whose_first_heredoc_is_unterminated_is_still_a_parse_error() {
    for script in [
        "cat <<E; cat <<T\nbodyT\nT",
        "echo a; cat <<E; echo b\nbody",
        "cat <<E",
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (out.as_str(), code),
            (
                if script.starts_with("echo") {
                    "a\n"
                } else {
                    ""
                },
                1
            ),
            "script={script:?} err={err}"
        );
        assert!(
            err.contains("missing terminating delimiter line 'E'"),
            "script={script:?} err={err}"
        );
    }
}

/// Task #275's **residual**, the block mirror of the acceptance shape (measured
/// 2026-10-21): the line's first operator (`<<T`, in the `then` arm) is the
/// unterminated one, so `found == 0` starves the complete `elif` body as well.
/// bash runs the `elif` and prints `b` (rc 0, two warnings) — #364.
#[test]
fn a_block_line_whose_first_operator_is_unterminated_is_still_a_parse_error() {
    let (out, err, code) = run("if false; then cat <<T; elif cat <<E; then echo b; fi\nbodyE\nE");
    assert_eq!((out.as_str(), code), ("", 1), "bash: b, rc 0");
    assert!(
        err.contains("missing terminating delimiter line 'E'"),
        "err={err}"
    );
}

/// Task #275, accepted divergence (measured 2026-10-21): a multi-line block
/// whose stream runs past the closer. The tail claim extends the extension-mode
/// span to the end of the text, so `fi` becomes body data and the block never
/// closes. bash consumes the same lines as body data and stops at a **syntax
/// error (rc 2)**; fdshell reports the missing delimiter (rc 1).
#[test]
fn a_multi_line_block_with_a_tail_claim_past_the_closer_is_a_parse_error() {
    let (out, err, code) = run("if true; then\ncat <<T\nT\ncat <<E\nfi");
    assert_eq!((out.as_str(), code), ("", 1), "bash rc 2, syntax error");
    assert!(
        err.contains("missing terminating delimiter line 'E'"),
        "err={err}"
    );
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. The fixed name
/// `fdshell-task238` was shared by every concurrent suite run, and pid
/// recycling inherits a dead run's dir (LESSONS, task #255; sweep task #342).
/// Removal is `.ok()`-tolerant: teardown never panics, and it runs on a
/// failing test's unwind, which is exactly the test that used to leak.
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

/// A pid + counter scratch dir `fdshell-heredoc-<tag>-<pid>-<counter>`,
/// removed when the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-heredoc-{tag}-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// Task #238's out-of-scope interaction, pinned at bash parity since task
/// **#298** fixed the family: a redirection written after a block's closer
/// belongs to the whole block, so the here-doc body (or the heredoc-free
/// output) lands in `out` and **nothing** reaches stdout. Before #298 the
/// trailing redirect was dropped silently — the body went to stdout and the
/// target was never even opened (`if true; then echo hi; fi > out` behaved that
/// way on master); the `for`/`case` twins were additionally the parse errors of
/// the untruncated closer search (`expected 'done'` / `case: missing 'esac'`,
/// heredoc-carrying and heredoc-free alike, task #273). Pinned: `> out` and its
/// `>>` twin for every block, plus the heredoc-free twins.
#[test]
fn single_line_block_body_heredoc_with_block_trailing_redirect() {
    let dir = scratch("task238");
    for (script, want) in [
        ("if true; then cat <<T; fi > out\nbodyT\nT", "bodyT\n"),
        ("for x in a; do cat <<T; done > out\nbodyT\nT", "bodyT\n"),
        ("for x in a; do cat <<T; done >> out\nbodyT\nT", "bodyT\n"),
        ("case x in x) cat <<T;; esac > out\nbodyT\nT", "bodyT\n"),
        ("case x in x) cat <<T;; esac >> out\nbodyT\nT", "bodyT\n"),
        // The heredoc-free twins.
        ("if true; then echo hi; fi > out", "hi\n"),
        ("for x in a; do echo A; done > out", "A\n"),
        ("case x in x) echo C;; esac > out", "C\n"),
    ] {
        let _ = std::fs::remove_file(dir.join("out"));
        let script = format!("cd {}; {}", dir.display(), script);
        let (out, err, code) = run(&script);
        // Every byte of the block's output went into `out`, not to stdout.
        assert_eq!((out.as_str(), code), ("", 0), "{script}: stderr={err:?}");
        assert_eq!(
            std::fs::read(dir.join("out")).unwrap(),
            want.as_bytes(),
            "{script}: bash writes {want:?} into `out`"
        );
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

/// Task #306 landed the **`for`** half of the span rule this pin used to carry:
/// a closer-shaped word of a `for` **word list** is word data, so the block's
/// span reaches its real `done` and the loop iterates over the word (bash's
/// answer, rc 0). The here-doc twin pins the same shape with a body region: the
/// list's `done` is a word, the clause keeps its region and the body prints.
/// `master` rejected both with `expected 'do'`, rc 1.
#[test]
fn closer_shaped_for_word_list_words_reach_the_real_closer() {
    let (out, err, code) = run("for x in done; do echo \"$x\"; done");
    assert_eq!((out.as_str(), code), ("done\n", 0), "stderr={err}");
    let (out, err, code) = run("for x in done; do cat <<T; echo body; done\nT\n");
    assert_eq!((out.as_str(), code), ("body\n", 0), "stderr={err}");
}

/// The **`case`** twin of
/// [`closer_shaped_for_word_list_words_reach_the_real_closer`], at bash parity
/// since task **#309**: the span scan is seeded with the block's own closer, so
/// a `done` of the `case` **pattern** list mismatches the `case`'s `esac` and is
/// word data — the block reaches its real `esac`, `done)` is a clause pattern and
/// bash's rc 0 follows (it was rc 1 on `master`, the third divergence of task
/// #273). The same rule lands in the **token** walk of `case_block` (the seed
/// `Closer::Esac`), so the token side agrees.
///
/// Task **#310** owns what is left of the span rule: a closer-shaped word of the
/// `for`/`case` list read at the **byte** level (`esac` behind a `for`, the
/// `case` list's `esac` in a nested span). The pair is pinned there.
#[test]
fn closer_shaped_case_word_list_words_reach_the_real_closer() {
    let (out, err, code) = run("case done in done) echo p;; esac");
    assert_eq!((out.as_str(), code), ("p\n", 0), "stderr={err}");
    let (out, err, code) = run("case done in done) cat <<T;; esac\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err}");
    let (out, err, code) = run("case done in esac");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
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

/// Task #273's second measured divergence, reshaped by task **#309**'s pairing:
/// a `; done` inside a clause body is a word (an `if`/`while` closer, not a
/// `case`'s), so it no longer ends the block — the clause keeps its region, its
/// body heredoc prints, and the clause's own statement then runs `done`, which
/// is no command: rc 1 with the resolve error, where bash is rc 2 (the rc-1-vs-2
/// family of `midrun_adjacent_shapes_stay_measured_divergences`). `master` read
/// the `done` as the block's closer and reported `missing 'esac'`, rc 1.
#[test]
fn case_body_heredoc_of_a_clause_terminated_by_done_runs_the_body() {
    let (out, err, code) = run("case x in x) cat <<T; done;; esac\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 1), "stderr={err:?}");
    assert!(
        err.contains("failed to resolve command path"),
        "stderr={err:?}"
    );
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
///
/// Task #295 adds the row the bug was filed with (no `echo mid` behind the
/// `cat`) and its `||` twin: the multi-line run is flushed on its own
/// physical line, so it is a single-line run for `flush_line` and claims the
/// line's whole region stream. Measured green since #232's own commit
/// `43ff10e` — at its parent the shape is not even parsed (`expected 'done'`).
#[test]
fn midrun_block_body_heredoc_is_region_scanned() {
    let (out, err, code) = run("true && while :; do cat <<T; echo mid; break; done\nbody\nT");
    assert_eq!((out.as_str(), code), ("body\nmid\n", 0), "stderr={err}");
    // Task #295's exact shape, and its short-circuit twin (no output, rc 0).
    let (out, err, code) = run("true && while :; do cat <<T; break; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("bodyT\n", 0), "stderr={err}");
    let (out, err, code) = run("true || while :; do cat <<T; break; done\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
    let (out, err, code) = run("cat <<X && echo b\nq && r\nX");
    assert_eq!((out.as_str(), code), ("q && r\nb\n", 0), "stderr={err}");
}

/// Task #295: the whole mid-run `<<` family is bash-equal. Every row is a
/// `(script, want_out)` pair measured against **bash 5.3.9** and
/// `target/debug/fdshell` on `master` (stdin `/dev/null`): the block is a cond
/// part, so its bytes stay inside the run and the run spans several physical
/// lines, yet the flush happens on the run's own line — the run claims that
/// line's region stream in operator order (`flush_line`, task #232's
/// mechanism). The `&&`-tail, the operator in the block's **condition**, two
/// `<<` in one body, the nested block, every keyword, the `|` stage (task
/// #292), the io-number form (task #276) and a here-string all travel with the
/// run; so does a `#` written behind the run, whose flush is on the run's own
/// line and never deferred.
///
/// The members that stay divergent are named by owner, never fixed here: a
/// **multi-line** mid-run construct whose region swallows its closer is rc 1
/// where bash is rc 2 (pinned in `tests/cond_blocks.rs`
/// `multi_line_mid_run_constructs_with_a_body_heredoc_stay_measured`, same
/// family as `block_body_heredoc_delimiter_line_operator_is_region_data`), a
/// group / function **body** here-doc claims no region (tasks **#315** /
/// **#305**, pinned below), a delimiter line that never arrives is task
/// **#364**, and the `#`-terminated / `&`-deferred flush is task **#333**.
#[test]
fn mid_run_heredoc_parity_family() {
    for (script, want) in [
        // The task's headline rows.
        (
            "true && while :; do cat <<T; break; done\nbodyT\nT",
            "bodyT\n",
        ),
        (
            "true && while :; do cat <<T; break; done && echo tail\nbodyT\nT",
            "bodyT\ntail\n",
        ),
        // The operator written in the block's own condition.
        ("true && while cat <<T; do break; done\nbodyT\nT", "bodyT\n"),
        // Two operators in one body: the line's stream is handed out in
        // operator order, and the second gets what is left of it.
        (
            "true && while :; do cat <<T; cat <<U; break; done\nbT\nbU\nT\nU",
            "bT\nbU\n",
        ),
        // A here-doc mid-run with no block at all, and one after the block.
        ("true && cat <<T; echo e\nbT\nT", "bT\ne\n"),
        ("true && while :; do break; done && cat <<T\nbT\nT", "bT\n"),
        // A block nested inside a mid-run block's body.
        (
            "true && while :; do if true; then cat <<T; fi; break; done\nbT\nT",
            "bT\n",
        ),
        // Every keyword opener, and the `if` with a `&&` behind the block.
        (
            "true && if true; then cat <<T; fi && echo y\nbT\nT",
            "bT\ny\n",
        ),
        ("true && case x in x) cat <<T;; esac\nbT\nT", "bT\n"),
        ("true && for i in 1; do cat <<T; done\nbT\nT", "bT\n"),
        // `until :` never enters its body, so nothing prints (rc 0).
        ("true && until :; do cat <<T; break; done\nbT\nT", ""),
        // The block as a pipeline stage (task #292).
        (
            "true | while :; do cat <<T; break; done\nbodyT\nT",
            "bodyT\n",
        ),
        // The delimiter forms.
        ("true && while :; do cat <<'T'; break; done\n$X\nT", "$X\n"),
        (
            "true && while :; do cat <<-T; break; done\n\tbT\n\tT",
            "bT\n",
        ),
        // The io-number body goes to fd 3, so stdin prints nothing (task #276).
        ("true && while :; do cat 3<<T; break; done\nbT\nT", ""),
        ("true && while :; do cat <<<str; break; done", "str\n"),
        // A `#` behind the run, and a comment line above it: the flush is on
        // the run's own line, so nothing is deferred (unlike #333's rows).
        (
            "true && while :; do cat <<T; break; done # c\nbodyT\nT",
            "bodyT\n",
        ),
        (
            "# c\ntrue && while :; do cat <<T; break; done\nbodyT\nT",
            "bodyT\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

// ── A `;`-terminated here-doc in a single-line `{ …; }` group body (task #315) ─
// The group branch claims the regions of the `<<` operators written in its own
// body text (`segment/block/claim.rs`, append mode) and the group builder
// re-attaches them to the stored body (`parse/part_text`), so the group parses
// and its body command receives the capture — the definer branch's rule
// (task #305) at the third block branch. Every expectation below was measured
// against bash 5.3.9 (`bash -c`) and `target/debug/fdshell`; rows named
// "divergence" reference their owner.

/// The headline: the group body's here-doc prints, the delimiter lines are
/// heredoc structure (never commands), the io-number form feeds fd 3 instead of
/// stdin, and a group that is a def body reaches the capture at the call.
#[test]
fn heredoc_in_a_single_line_group_body_prints_its_region() {
    for (script, want) in [
        ("{ cat <<T; }\nbodyT\nT", "bodyT\n"),
        ("{ cat 3<<T; }\nbodyT\nT", ""),
        ("f() { { cat <<T; }; }; f\nbodyT\nT", "bodyT\n"),
        ("{ cat <<T; }\nbodyT\nT\necho tail", "bodyT\ntail\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// A region's **lines are data**, and the delimiter word is a command only after
/// the region consumed its body: `}`, a keyword word, an opener, a `;`, a
/// cond-operator line all print verbatim. The body region's words are blanked by
/// the tokenizer (task #266), so none of them reaches the parser as a word.
#[test]
fn region_lines_of_a_group_body_heredoc_are_data() {
    for (script, want) in [
        ("{ cat <<T; }\n}\nT", "}\n"),
        ("{ cat <<T; }\ndone\nT", "done\n"),
        ("{ cat <<T; }\n{\nT", "{\n"),
        ("{ cat <<T; }\n;\nT", ";\n"),
        ("{ cat <<T; }\nb && y\nT", "b && y\n"),
        ("{ cat <<T; }\nf() { h; }\nT", "f() { h; }\n"),
        ("{ cat <<T; }\n\nT", "\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
    // An empty body: the delimiter line is the first line, so nothing prints and
    // the **next** line runs as a command. bash: rc 127, `T: command not found`;
    // fdshell reports a command-not-found as rc 1 (the #305 twin's rule), so the
    // row pins the empty output and the empty body, not bash's code.
    let (out, _err, code) = run("{ cat <<T; }\nT\nT");
    assert_eq!((out.as_str(), code), ("", 1), "bash prints nothing, rc 127");
}

/// The delimiter forms ride along unchanged: quoted, space-bearing, `<<-`, and
/// the chevron-shaped delimiter of task #266.
#[test]
fn group_body_heredoc_delimiter_forms() {
    for (script, want) in [
        ("{ cat <<'T'; }\n$HOME\nT", "$HOME\n"),
        ("{ cat <<\"a b\"; }\nbody\na b", "body\n"),
        ("{ cat <<'a b'; }\nbody\na b", "body\n"),
        ("{ cat <<-T; }\n\tbody\n\tT", "body\n"),
        ("{ cat <<'a <<b'; }\nbody\na <<b", "body\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The here-doc does not swallow the statements around the group, and every
/// operator of the line takes **its own** region of the line's stream in
/// operator order (`parse/cond_bodies.rs`'s skip, the rule task #240 pins at the
/// scan level). The mid-run group (`true && { … }`) is a cond part, so it rides
/// the run's bodies through the same cond/parse chain.
#[test]
fn group_body_heredoc_keeps_the_operators_around_the_group() {
    for (script, want) in [
        ("{ cat <<T; }; echo t\nbody\nT", "body\nt\n"),
        ("{ cat <<T; }; cat <<U\nbT\nT\nbU\nU", "bT\nbU\n"),
        ("cat <<T; { cat <<U; }\nbT\nT\nbU\nU", "bT\nbU\n"),
        ("{ cat <<T; }; { cat <<U; }\nbT\nT\nbU\nU", "bT\nbU\n"),
        ("{ cat <<T; } && echo b\nbody\nT", "body\nb\n"),
        ("{ cat <<T; } || echo b\nbody\nT", "body\n"),
        ("{ cat <<T && echo b; }\nbody\nT", "body\nb\n"),
        ("true && { cat <<T; }\nbodyT\nT", "bodyT\n"),
        ("{ :; } && { cat <<T; }\nbody\nT", "body\n"),
        ("{ cat <<T; } && { cat <<U; }\nbT\nT\nbU\nU", "bT\nbU\n"),
        ("{ cat <<T; } > /dev/null && echo t\nbody\nT", "t\n"),
        ("{ cat <<T; }\nbody\nT\necho tail", "body\ntail\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The statement-level twin of the #332 row: a here-doc statement **before** the
/// group on the same line, and a statement after the group's own `;`. The group's
/// span ends its statement, and the line's stream is already fully claimed by the
/// time the scan reaches the group, so the trailing statement is dropped —
/// fdshell prints the two bodies where bash prints them **and** `t` (task
/// **#332**, the same drop `heredoc_block_then_group_then_statement_on_one_line`
/// pins for a group with no heredoc of its own).
#[test]
fn group_body_heredoc_behind_a_statement_drops_the_trailing_statement() {
    let (out, _err, code) = run("cat <<X; { cat <<T; }; echo t\nbodyX\nX\nbodyT\nT");
    assert_eq!(
        (out.as_str(), code),
        ("bodyX\nbodyT\n", 0),
        "bash prints bodyX, bodyT and t at rc 0 (task #332)"
    );
}

/// Measured divergences of the group-body shape that stay with their owners:
///
/// * the **tail** `{ …; } <<EOF` (the group's own here-doc): bash feeds the body
///   to the group, fdshell keeps its `here-doc on a group is not supported`
///   message (tasks **#187**/**#191**). The claim never reaches the tail: the tail
///   operator's region is not part of the group's body text.
/// * the **EOF-starved** stream: the region's delimiter line never arrives, so
///   the line's allocation claims nothing (task **#429**'s family) and fdshell
///   rejects the group where bash prints the body with its end-of-file warning.
/// * the delimiter word shaped like a brace (`{ cat <<}; }`): the closer search
///   of task **#432** — bash prints `x` rc 0 with its EOF warning, fdshell
///   reports the missing delimiter for the `}` word.
/// * the **multi-line** group with a `;`-terminated inner heredoc: bash rejects
///   the syntax at rc 2, fdshell runs the first body line and reports the missing
///   delimiter at rc 1 (the rc-1-vs-2 family).
/// * the group as a pipeline stage: `} | cat` is unsupported (tasks
///   **#186**/**#190**, pinned as `unexpected pipe` by
///   `tests/group.rs::pipeline_stage_group_is_a_parse_error`).
#[test]
fn group_body_heredoc_divergences_keep_their_owners() {
    for (script, want) in [
        (
            "{ cat <<T; } <<EOF\nbody\nT\nEOF",
            "here-doc on a group is not supported",
        ),
        (
            "{ cat <<T; }\nbody",
            "missing terminating delimiter line 'T'",
        ),
        ("{ cat <<}; }\nx", "missing terminating delimiter line '}'"),
        (
            "{ echo a\ncat <<T;\n}\nbody\nT",
            "missing terminating delimiter line 'T'",
        ),
        ("{ cat <<T; } | cat\nbody\nT", "unexpected pipe"),
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: stderr={err}");
        assert!(err.contains(want), "{script}: stderr={err}");
    }
}

// ── A `;`-terminated here-doc in a function-definition body (task #305) ──────
// The definer's own operators claim their regions of the line's body stream
// (`segment/block/claim.rs`, append mode) and the stored body carries them
// (`parse/part_text`), so the definition parses and every **call** prints the
// captured content — bash reaches the same by parsing the input once. Every
// expectation below was measured against bash 5.3.9 (`bash -c`) and
// `target/debug/fdshell`; rows named "divergence" reference their owner.

/// The definer-body here-doc prints on the first call and on every later one,
/// and the definition may end the line it is written on (the call on a later
/// line sees the same capture).
#[test]
fn heredoc_of_a_terminated_function_body_line_prints_on_every_call() {
    for (script, want) in [
        ("f() { cat <<E; }; f\nbody\nE", "body\n"),
        ("f() { cat <<E; }; f; f\nbody\nE", "body\nbody\n"),
        ("f() { cat <<E; }; f; f; f\nbody\nE", "body\nbody\nbody\n"),
        ("f() { cat <<E; }\nbody\nE\nf", "body\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The here-doc does not swallow the rest of the def body, the trailing runs of
/// the defining line, or the status of the call.
#[test]
fn heredoc_of_a_terminated_function_body_line_keeps_the_rest_of_the_body() {
    for (script, want) in [
        ("f() { cat <<E; echo z; }; f\nbody\nE", "body\nz\n"),
        (
            "f() { cat <<E; }; f; echo tail\nbody\nE\necho after",
            "body\ntail\nafter\n",
        ),
        ("f() { cat <<E; }; f; echo rc=$?\nbody\nE", "body\nrc=0\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The `&&` twin: the definer's `{` is a group opener for the cond layer (task
/// #354), so the body's `&&` never splits the definition, and the chain runs on
/// every call.
#[test]
fn and_twin_of_a_terminated_function_body_heredoc_runs_the_chain() {
    for (script, want) in [
        ("f() { cat <<E && echo b; }; f\nbody\nE", "body\nb\n"),
        (
            "f() { cat <<E && echo b; }; f; f\nbody\nE",
            "body\nb\nbody\nb\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// Two operators in one def body take their regions in **operator order**, and
/// two definers on one line share the line's stream the same way. The
/// multi-line form of the `function` keyword stays verbatim (extension mode).
#[test]
fn two_terminated_function_body_heredocs_take_their_regions_in_operator_order() {
    for (script, want) in [
        ("f() { cat <<E; cat <<F; }; f\none\nE\ntwo\nF", "one\ntwo\n"),
        (
            "f() { cat <<E; }; f; g() { cat <<F; }; g\none\nE\ntwo\nF",
            "one\ntwo\n",
        ),
        ("function f { cat <<EOF\nhi\nEOF\n}; f", "hi\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The definer's own region sits in the **same** line stream as a pre-block and
/// a trailing statement's regions: each operator takes its own region, in the
/// global order (the family task #240 pins at the scan level).
#[test]
fn function_body_heredoc_shares_the_line_stream_with_its_neighbours() {
    for (script, want) in [
        (
            "cat <<X; f() { cat <<E; }; f\nbodyX\nX\nbodyE\nE",
            "bodyX\nbodyE\n",
        ),
        (
            "f() { cat <<E; }; cat <<G; f\nbodyE\nE\nbodyG\nG",
            "bodyG\nbodyE\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// A region's **lines are data**, never commands: `b && y`, a closer-shaped
/// `}`, a keyword word, an opener, a `;`, a definer, a blank line, and an empty
/// body (the first line is the delimiter) all print verbatim. The body region's
/// words are blanked by the tokenizer (task #266), so none of them reaches the
/// parser as a word.
#[test]
fn region_lines_of_a_function_body_heredoc_are_data() {
    for (script, want) in [
        ("f() { cat <<E; }; f\nb && y\nE", "b && y\n"),
        ("f() { cat <<E; }; f\n}\nE", "}\n"),
        ("f() { cat <<E; }; f\ndone\nE", "done\n"),
        ("f() { cat <<E; }; f\n{\nE", "{\n"),
        ("f() { cat <<E; }; f\n;\nE", ";\n"),
        ("f() { cat <<E; }; f\ng() { h; }\nE", "g() { h; }\n"),
        ("f() { cat <<E; }; f\n\nE", "\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
    // An empty body: the delimiter line is the first line, so nothing prints and
    // the **next** line runs as a command. bash: rc 127, `E: command not found`;
    // fdshell reports a command-not-found as rc 1 (measured on master for a bare
    // `E`), so the row pins the output and the empty body, not bash's code.
    let (out, _err, code) = run("f() { cat <<E; }; f\nE\nE");
    assert_eq!(
        (out.as_str(), code),
        ("", 1),
        "empty body: bash prints nothing, rc 127"
    );
}

/// A definition written inside a keyword-block arm, a `case` clause, or behind
/// a statement on the same line: the arm text is rebuilt with its regions
/// (`parse/part_text` runs at the arm granularity too), the definition lands in
/// the shell, and the call outside the block prints the body. A trailing block
/// on the defining line must not re-scan the definition's regions (the line's
/// floor, pinned at the scan level by
/// `segment/tests.rs::scan_trailing_block_after_a_function_heredoc_keeps_the_line_floor`).
#[test]
fn function_body_heredoc_defined_inside_a_block_arm_prints_at_the_call() {
    for (script, want) in [
        ("if :; then f() { cat <<E; }; fi\nbody\nE\nf", "body\n"),
        ("case x in x) f() { cat <<E; };; esac\nbody\nE\nf", "body\n"),
        (
            "while :; do f() { cat <<E; }; break; done\nbody\nE\nf",
            "body\n",
        ),
        ("f() { cat <<E; }; case x in x) f;; esac\nbody\nE", "body\n"),
        ("f() { cat <<E; }; if :; then f; fi\nbody\nE", "body\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// A definition nested in a definition: the inner def body is text at definition
/// time, so its own region rides along to the call (task #302's def-site rule).
#[test]
fn nested_function_definitions_capture_their_body_heredoc() {
    let (out, err, code) = run("f() { g() { cat <<E; }; g; }; f\nbody\nE");
    assert_eq!((out.as_str(), code), ("body\n", 0), "stderr={err}");
}

/// A `{ …; }` group written **behind** a block whose here-doc region the line
/// carries, with a statement after the group's own `;`: the group's span ends
/// its **statement**, not the physical line, so it keeps its own resume and the
/// trailing statement runs (`Book::rewind`'s `line_ends` rule, task #305). The
/// group's **own** body claim landed with task **#315**
/// (`heredoc_in_a_single_line_group_body_prints_its_region`); the
/// pre-block-statement twin below is **#332**'s: fdshell drops the statement
/// after the group there exactly as it does on master.
#[test]
fn heredoc_block_then_group_then_statement_on_one_line() {
    for (script, want) in [
        (
            "if :; then cat <<E; fi; { :; }; echo t\nbody\nE",
            "body\nt\n",
        ),
        ("f() { cat <<E; }; { :; }; echo t\nbody\nE", "t\n"),
        (
            "if :; then cat <<E; fi; { :; } && echo t\nbody\nE",
            "body\nt\n",
        ),
        ("if :; then cat <<E; fi; { :; }\nbody\nE", "body\n"),
        (
            "{ :; }; if :; then cat <<E; fi; echo t\nbody\nE",
            "body\nt\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
    // The #332 row: the group's line-start rewind drops the trailing statement.
    let (out, _err, code) = run("cat <<E; { :; }; echo t\nbody\nE");
    assert_eq!(
        (out.as_str(), code),
        ("body\n", 0),
        "bash prints body and t at rc 0 (task #332)"
    );
}

/// The delimiter forms ride along unchanged: quoted, space-bearing, `<<-`, and
/// the `function` keyword definer.
#[test]
fn function_body_heredoc_delimiter_forms() {
    for (script, want) in [
        ("f() { cat <<'E'; }; f\n$HOME\nE", "$HOME\n"),
        ("f() { cat <<\"a b\"; }; f\nbody\na b", "body\n"),
        ("f() { cat <<-E; }; f\n\tbody\n\tE", "body\n"),
        ("function f { cat <<E; }; f\nbody\nE", "body\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// The io-number form (task #276) in a def body: the region feeds fd 3, so
/// stdout stays empty — bash's row exactly.
#[test]
fn numbered_heredoc_in_a_function_body_feeds_the_descriptor() {
    let (out, err, code) = run("f() { cat 3<<T; }; f\nbodyT\nT");
    assert_eq!((out.as_str(), code), ("", 0), "stderr={err}");
}

/// Calls in a loop, behind a redirect, in a pipeline inside the body, and an
/// empty body: every call re-runs the **captured** text.
#[test]
fn function_body_heredoc_calls_in_a_loop_and_with_redirects() {
    for (script, want) in [
        ("f() { cat <<E; }; f > /dev/null; f\nbody\nE", "body\n"),
        ("f() { cat <<E | wc -l; }; f\na\nb\nE", "2\n"),
        ("f() { cat <<E; }; f\nE\necho tail", "tail\n"),
        ("f() { cat <<E; }; f\nbody\nE\necho tail", "body\ntail\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((out.as_str(), code), (want, 0), "{script}: stderr={err}");
    }
}

/// Measured divergences that stay with their owners:
///
/// * the loop row: fdshell's `while` reports the **failing condition's** status
///   (rc 1) where bash reports the last body command's (rc 0) — measured on
///   master with a plain loop body, so it is the loop-status rule, not this
///   task's; the here-doc itself prints `body` twice.
/// * the incomplete stream: the line's **first** operator's delimiter line never
///   arrives, so the allocation claims nothing (task **#429**'s family) and
///   fdshell rejects the definition where bash prints the body with its
///   end-of-file warning.
/// * the unclosed definer keeps its missing-closer message (task #277's
///   contract), and the group body row stays with **#315**.
#[test]
fn function_body_heredoc_of_an_incomplete_line_stream_is_a_divergence() {
    let (out, _err, code) =
        run("f() { cat <<E; }; i=0; while [ $i != 2 ]; do f; i=$((i+1)); done\nbody\nE");
    assert_eq!(
        (out.as_str(), code),
        ("body\nbody\n", 1),
        "bash prints the same twice at rc 0"
    );
    for (script, want) in [
        (
            "f() { cat <<E; }; f\nbody",
            "missing terminating delimiter line 'E'",
        ),
        ("f() { cat <<E;", "is missing its closing '}'"),
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "{script}: bash prints the body at rc 0");
        assert!(err.contains(want), "{script}: stderr={err}");
    }
}

/// The shapes the definer claim does **not** reach, pinned with their owners so
/// the next reader does not half-fix them:
///
/// * a delimiter word shaped like a brace (`f() { cat <<}; }; f`): bash prints
///   the body rc 0 (with its end-of-file warning), fdshell rejects the line —
///   the closer search of task **#432**.
/// * a function in a pipeline stage (`f | cat`): the stage's process has no
///   function table (task **#182**).
/// * a def-prefix redirection (`f() <<E { … }`): bash rejects the syntax rc 2;
///   fdshell parses it as a definer and the def body's own region prints
///   (task **#433**, filed by this one — #180 is the accepted trailing form).
#[test]
fn function_body_heredoc_shapes_that_stay_divergent() {
    let (out, _err, code) = run("f() { cat <<}; }; f\nx");
    assert_eq!((out.as_str(), code), ("", 1), "bash prints `x` at rc 0");
    let (out, _err, code) = run("f() { cat <<E; }; f | cat\nbody\nE");
    assert_eq!(
        (out.as_str(), code),
        ("", 0),
        "bash prints body at rc 0 (task #182)"
    );
    let (out, _err, code) = run("f() <<E { cat <<G; }\nbodyE\nE\nbodyG\nG\nf");
    assert_eq!(
        (out.as_str(), code),
        ("bodyG\n", 0),
        "bash rejects the def-prefix redirection at rc 2 (task #433)"
    );
}

/// A script file whose last delimiter line has **no trailing newline**: the
/// region bytes are byte-exact (`scan/heredoc/full.rs::region_bytes`), so the
/// body is captured and printed exactly as with `-c`.
#[test]
fn function_body_heredoc_at_end_of_input_without_a_trailing_newline() {
    let path = std::env::temp_dir().join(format!("fdshell-it-heredoc-305-{}", std::process::id()));
    std::fs::write(&path, "f() { cat <<E; }; f\nbody\nE").unwrap();
    let output = Command::new(BIN)
        .arg(&path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    let stdout = str::from_utf8(&output.stdout).unwrap().to_string();
    let stderr = str::from_utf8(&output.stderr).unwrap().to_string();
    assert_eq!(
        (stdout.as_str(), output.status.code().unwrap_or(-1)),
        ("body\n", 0),
        "stderr={stderr}"
    );
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

    // `( … )` is not a subshell: `(` reaches the command lookup (task #316).
    let (out, err, code) = run("( cat 3<<T )\nbody\nT");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("\"(\" not found"), "{err}");
}

// --- Task #261: the byte/token `<<`-count residuals of task #185. Four shape
// classes were left diverging by the `seen_word` → `seen_token` fix: an escaped
// or quoted `|` before the operator, an `((`-leading run, an invalid/missing
// delimiter word, and a newline-leading run. Every row below is measured
// against bash 5.3.9 (stdin `/dev/null`); the bash value is in the comment, and
// the shapes bash treats as a syntax error (rc 2) stay in fdshell's rc-1
// families. Nothing here changes the delimiter-validity errors or #185's pins.
// ---

/// An escaped `|` is one word (`x\|` is word content for the tokenizer), so the
/// `<<` behind it is an operator, not a pipeline command word: the run claims
/// its body and the command prints its word. Master counted 0/1 and reported
/// `missing terminating delimiter line 'EOF'`; bash prints the word (rc 0).
#[test]
fn heredoc_escaped_pipe_predecessor_is_a_word_not_a_stage() {
    for (script, want) in [
        ("echo x\\| <<EOF\nbody\nEOF", "x|\n"),
        ("echo \\| <<EOF\nbody\nEOF", "|\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// The quoted twin, measured byte 1 / token 0 on master (the token layer read
/// the *text* of the preceding token, and `"|"`/`'|'` are text `|`): `is_operator`
/// and the pipeline split now read the token's raw span. bash prints the word
/// (rc 0), and so does fdshell.
#[test]
fn heredoc_quoted_pipe_predecessor_is_a_word_not_a_stage() {
    for (script, want) in [
        ("echo \"|\" <<EOF\nbody\nEOF", "|\n"),
        ("echo '|' <<EOF\nbody\nEOF", "|\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// The pipeline twin of the quoted case: a quoted `>` never terminates a word,
/// so the `|` **is** a pipeline pipe and the shape stays in the pipeline family
/// (`<<EOF` becomes the second stage's command word). Unchanged by task #261 at
/// both levels (byte 0 / token 0, pinned in
/// `parse/heredoc/tests.rs::operator_count_pipe_predecessors_agree`); bash runs
/// the pipeline and its rc is 0.
#[test]
fn heredoc_quoted_gt_word_keeps_the_pipeline_split() {
    let (out, err, code) = run("echo \"a>\"| <<EOF\nbody\nEOF");
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("\"<<EOF\" not found"), "stderr={err:?}");
    // No heredoc operator on the line, so no region is claimed and the body
    // lines are the pipeline's own commands (`tests/redirect.rs::quoted_operator_word_keeps_the_pipe_and_the_body_free`).
    assert!(err.contains("\"body\" not found"), "stderr={err:?}");
    let (out, err, code) = run("cat \"a>\"| <<EOF\nbody\nEOF");
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("cat: 'a>'"), "stderr={err:?}");
}

/// The `(( … ))` region stays opaque to the byte scan (its `<<` is counted by
/// the token layer only, and `arith_cmd` intercepts the line before any heredoc
/// allocation). Every shape here is measured byte 0 / token 1 and stays the
/// arithmetic parse error, rc 1 (bash rc 2); the body lines never run.
#[test]
fn heredoc_arith_command_region_stays_opaque_to_the_byte_scan() {
    for script in [
        "(( <<EOF\nbody\nEOF",
        "(( 1 <<EOF\nbody\nEOF",
        "(( x <<EOF\nbody\nEOF",
        "(( x ; cat <<B\nbody\nB",
        "(( x ; cat <<B; fi\nbody\nB",
        "true && (( x ; cat <<B\nbody\nB",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (1, ""), "script={script:?}");
        assert!(
            err.contains("arithmetic command must be `((expr))` with no trailing words"),
            "stderr={err:?}"
        );
        assert!(
            !err.contains("\"body\" not found"),
            "body must not leak: {err:?}"
        );
    }
}

/// The load-bearing half of the same rule: the `$( … )` and closed `(( … ))`
/// regions are opaque, so their heredocs keep their bodies. Counting the
/// arithmetic `<<` (what `is_operator` counts) would empty the line's region
/// stream and steal `cat <<B`'s body; these are the rc-0 pins that gate it.
/// bash 5.3.9 agrees on every row except the last, where it warns about the
/// inner heredoc and exits 1.
#[test]
fn heredoc_substitution_and_closed_arith_regions_keep_their_bodies() {
    for (script, want) in [
        ("(( 1 << 2 )) && cat <<B\nbody\nB", "body\n"),
        ("(( 1 << 2 )); cat <<B\nbody\nB", "body\n"),
        ("x=$(cat <<EOF\nbody\nEOF\n); echo \"$x\"", "body\n"),
        ("echo \"$(cat <<EOF\nb\nEOF\n)\"", "b\n"),
        ("cat <<A\n$(echo x <<Q\nq\nQ)\nA", "x\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(
            (code, out.as_str()),
            (0, want),
            "script={script:?} err={err:?}"
        );
    }
}

/// Class 3: a bare `<<` whose delimiter word starts with `<`, `>`, `&`, `%` (or
/// that has no delimiter at all) is counted at both levels and stays the
/// parse-layer syntax error. The count no longer zeroes the window, the region
/// layer keeps its `None`, and every message is what master printed (the
/// window-subset invariant is what makes the split allocation-neutral).
/// bash is rc 2 on all of them; `cat << %x` is rc 0 there (a warning only).
#[test]
fn heredoc_invalid_delimiter_keeps_the_parse_errors() {
    for script in [
        ">> << <<EOF\nbody\nEOF",
        "cat <<- <<EOF\nbody\nEOF",
        "cat << <<A && cat <<B\nbody\nA\nB",
        "cat <<- <<EOF && cat <<B\nbody\nEOF\nB",
        ">> << <<EOF && cat <<B\nbody\nB",
        "if true; then cat <<- <<X; fi\nbody\nX",
        "cat << <x\nbody\nx",
        "cat << &x\nbody\nx",
        "cat << %x\nbody\nx",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (1, ""), "script={script:?}");
        assert!(err.contains("invalid redirect syntax"), "stderr={err:?}");
    }
    // The multi-part line: the first operator's delimiter line is what the
    // parse reports, unchanged, and no body line is executed.
    let (out, err, code) = run("cat <<A ; cat <<- <<X ; cat <<B\na\nA\nX\nB");
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(
        err.contains("missing terminating delimiter line 'A'"),
        "stderr={err:?}"
    );
    assert!(
        !err.contains("\"X\" not found"),
        "body must not leak: {err:?}"
    );
}

/// Class 4: the tokenizer's separator arm emits a `;` token for a newline, so a
/// newline-leading run has a preceding token, claims its body region, and
/// reports **one** error (its operator word) where master ran the body lines as
/// commands (`"<<EOF"`, `"body"`, `"EOF"`). rc stays 1; bash is rc 0 for the
/// "heredoc with no command word" family (task #345 owns that gap), and for the
/// `x` row it is the `x: command not found` status the heredoc replaces.
#[test]
fn heredoc_leading_newline_consumes_its_body() {
    let (out, err, code) = run("\n<<EOF\nbody\nEOF");
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("\"<<EOF\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"body\" not found"),
        "body must not leak: {err:?}"
    );
    let (out, err, code) = run("true &&\n<<B\nbody\nB");
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("\"<<B\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"body\" not found"),
        "body must not leak: {err:?}"
    );
    let (out, err, code) = run("x\n<<EOF\nbody\nEOF");
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("\"x\" not found"), "stderr={err:?}");
    assert!(err.contains("\"<<EOF\" not found"), "stderr={err:?}");
    assert!(
        !err.contains("\"body\" not found"),
        "body must not leak: {err:?}"
    );
}

/// Task #313: the shapes behind a plain `&` that fdshell and bash still
/// disagree on, pinned **as measured** (never silently accepted) with the owner
/// named. Every row was measured against `bash -c` on 2026-10-20.
///
/// * A `#` comment behind the `&`: the comment ends the physical line
///   (`Book::resume_past`), so the run behind the `&` trails its own line and
///   is flushed body-less — the rule
///   `segment/line/tests.rs::flush_line_multi_line_runs_are_body_less` pins,
///   and the twin `cat <<A # note` (no `&`) has always reported the same way.
///   fdshell rc 1, bash rc 0 with both bodies.
/// * Glued `<<A&`: `&` is a **word byte** for the tokenizer
///   (`scan::heredoc::lines::token_word_end` says so explicitly), so the
///   byte-level delimiter is `A&` and the line's region list is empty. Owner:
///   the glued-pair family (the tokenizer's split set), not this task.
/// * A redirect-only suffix (`<<B` / `<<U` with no command word): bash runs a
///   bare redirection; fdshell runs `<<B` as a command word. Owner **#345** —
///   the statement carries a here-doc body, which is exactly the condition
///   task #303's parse gate stays off for (the plain `>f` family is rc 0 now).
#[test]
fn a_heredoc_behind_an_ampersand_known_divergences() {
    // The `#`-behind-`&` row and its glued twin: the prefix job runs (its body
    // prints), the suffix is the parse error.
    for (script, delim) in [
        ("cat <<A & cat <<B # note\nx\nA\ny\nB", "B"),
        ("cat <<A& cat <<B\nx\nA\ny\nB", "B"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "script {script:?}: out={out}");
        assert!(
            err.contains(&format!("missing terminating delimiter line '{delim}'")),
            "script {script:?}: stderr={err}"
        );
    }
    // The redirect-only suffix: the prefix's body prints, the suffix is a
    // command lookup (bash: rc 0, no complaint).
    for (script, word) in [
        ("cat <<A & <<B\nx\nA\ny\nB", "\"<<B\""),
        ("echo hi <<T & <<U\nbody\nT\nb2\nU", "\"<<U\""),
    ] {
        let (out, err, code) = run(script);
        assert_eq!(code, 1, "script {script:?}: out={out}");
        assert!(
            err.contains(&format!("failed to resolve command path: {word}")),
            "script {script:?}: stderr={err}"
        );
    }
}

/// Task #313, the all-or-nothing rows of the same family. A physical line's
/// region list is computed whole: an operator whose body is missing, or whose
/// delimiter cannot be read, takes the **whole line's** list with it, so every
/// statement on that line is body-less and the parse layer reports the shape
/// (owner **#275**). bash reads to EOF with a warning and runs on.
#[test]
fn a_heredoc_behind_an_ampersand_reports_a_missing_body() {
    // The first body never arrives: `A` is the report (bash: two EOF warnings,
    // rc 0).
    let (out, err, code) = run("cat <<A & cat <<B\ny\nB");
    assert_eq!(code, 1, "out={out}");
    assert!(
        err.contains("missing terminating delimiter line 'A'"),
        "stderr={err}"
    );
    // `cat <<A &` with no body lines at all: bash warns at EOF and exits 0.
    let (_out, err, code) = run("cat <<A &");
    assert_eq!(code, 1);
    assert!(
        err.contains("missing terminating delimiter line 'A'"),
        "stderr={err}"
    );
    // A bare `<<` behind the `&`: bash's rc 2 syntax error, fdshell's rc 1
    // `invalid redirect syntax` — the same rc family, after the fix the prefix
    // prints its body first.
    let (out, err, code) = run("cat <<A & cat <<\nx\nA");
    assert_eq!(code, 1, "out={out}");
    assert!(err.contains("invalid redirect syntax"), "stderr={err}");
    // Unterminated bodies next to the `&` (the pre-existing heredoc rule,
    // owner #266 for the terminator-is-statement-text case): bash warns and
    // reads to EOF, fdshell bails.
    for script in [
        "echo hi <<T &\nbody\n T\necho after",
        "echo hi <<T &\nbody\nT; echo after",
        "echo hi <<T &\r\nbody\r\nT\r\n",
    ] {
        let (_out, err, code) = run(script);
        assert_eq!(code, 1, "script {script:?}");
        assert!(
            err.contains("missing terminating delimiter line 'T'"),
            "script {script:?}: stderr={err}"
        );
    }
}

/// Task #313, the deferral and block-behind-`&` rows (measured 2026-10-20):
///
/// * `cat <<A\n & cat <<B` — the line-deferral of task #326 (a newline whose
///   next line opens with `&`): fdshell attaches both bodies and runs the pair
///   (rc 0), bash runs the line as garbage (`"y" not found`, rc 127). fdshell
///   is the *nicer* side here; the row pins it so the deferral cannot regress
///   into the junk shape.
/// * `cat <<A & cat <<B` + a third heredoc line below: both shells produce
///   junk (the body of the third operator lands in the middle of the run).
/// * `cat <<A & if true; then cat <<B … fi` — a **block opened behind the
///   `&`**: the block's span necessarily covers the prefix's body lines, so
///   they become statements of the block body (`x`, `A` printed twice). Owners
///   **#352**/**#316** (the one-job async list family); bash prints both bodies.
#[test]
fn a_heredoc_behind_an_ampersand_deferral_and_block_rows() {
    let (out, err, code) = run("cat <<A\n & cat <<B\nx\nA\ny\nB");
    assert_eq!(code, 0, "out={out} stderr={err}");
    let mut lines: Vec<&str> = out.lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, ["x", "y"], "stdout={out}");

    let (out, _err, code) = run("cat <<A & cat <<B\ncat <<C\nx\nA\ny\nB\nz\nC");
    assert_eq!(code, 1);
    // The job's output has no order, so the lines are a set (LESSONS).
    let mut lines: Vec<&str> = out.lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, ["cat <<C", "x", "y"], "stdout={out}");

    let (out, _err, code) = run("cat <<A & if true; then cat <<B\nx\nA\ny\nB\nfi");
    assert_eq!(code, 0);
    // Set again: the backgrounded prefix and the block body interleave.
    let mut lines: Vec<&str> = out.lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, ["A", "x", "x", "y"], "stdout={out}");
}

/// Task #313, the case-arm rows. `case x in x) echo hi <<T & ;; esac` with the
/// body **below** runs the arm's statement with its body (bash rc 0, identical
/// — the block-text `&` of task #354); with the `case` unclosed (`& ;;` and no
/// closer behind) fdshell reports the missing terminator where bash reports its
/// rc 2 EOF syntax error (the block-text `& ;;` family, #354/#355).
#[test]
fn a_heredoc_at_the_end_of_a_case_arm_with_an_ampersand() {
    let (out, err, code) = run("case x in x) echo hi <<T & ;; esac\nbody\nT");
    assert_eq!(code, 0, "stderr={err}");
    assert_eq!(out, "hi\n");

    let (_out, err, code) = run("case x in x) echo hi <<T & ;;\nesac");
    assert_eq!(code, 1);
    assert!(
        err.contains("missing terminating delimiter line 'T'"),
        "stderr={err}"
    );
}

// --- Task #266: a here-doc **terminator line** is heredoc structure, never
// command bytes. The delimiter word is a word (task #207), so it may contain a
// `<<`, an unbalanced quote byte, a `|`, a substitution — and the line that
// carries it, which travels inside the statement text, must stay opaque to the
// statement layers. Every row below is measured against bash 5.3.9 and was rc 1
// on `master` (the terminator's own `<<` counted as an operator at both levels,
// shifting the global per-line body allocation; its quote byte reaching the
// tokenizer's quote check; its `|` splitting a pipeline).
// ---

/// A terminator line whose delimiter word contains a chevron is data: the
/// command prints its body, rc 0. `"` twin and the trailing-statement twin of
/// the same shape.
#[test]
fn a_terminator_line_with_a_chevron_is_data() {
    for (script, want) in [
        ("cat <<'a <<b'\nbody\na <<b", "body\n"),
        ("cat <<\"a <<b\"\nbody\na <<b", "body\n"),
        ("cat - <<'a <<b'\nbody\na <<b", "body\n"),
        ("cat <<'a <<b'\nbody\na <<b\necho after", "body\nafter\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// The escaped-quote delimiter rule (task #207) produces a terminator line with
/// an **unbalanced** quote byte; such a line must never reach the tokenizer's
/// quote check, exactly as an unbalanced body line does not.
#[test]
fn a_terminator_line_with_an_unbalanced_quote_is_data() {
    for script in [
        "cat <<a\\\"b\nbody\na\"b",
        "cat <<'a\"b'\nbody\na\"b",
        "cat <<'a$(b'\nbody\na$(b",
        "cat <<'a`b'\nbody\na`b",
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, "body\n"), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// The mask decision in `tokenize_statement` keys on the **body** regions, not
/// on the terminator list: an empty delimiter line (`<<''`) has no bytes of its
/// own, so it yields no terminator span, and its body is opaque data exactly
/// like any other body — an unbalanced quote, a pipeline, an unterminated `$(`.
#[test]
fn the_body_of_an_empty_delimiter_line_is_data() {
    for (script, want) in [
        ("cat <<''\n'\n\n", "'\n"),
        ("cat <<''\n\"x\" | cat\n\n", "\"x\" | cat\n"),
        ("cat <<''\n$(\n\n", "$(\n"),
        ("cat <<''\nbody\n\necho after", "body\nafter\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// A `|` on the terminator line is not a pipeline pipe, and a chevron
/// delimiter in a pipeline stage keeps the stage: the terminator is the stage's
/// own data.
#[test]
fn a_terminator_line_with_a_pipe_is_not_a_pipeline() {
    for (script, want) in [
        ("cat <<'a|b'\nbody\na|b", "body\n"),
        ("cat <<'a <<b' | cat\nbody\na <<b", "body\n"),
        ("cat <<'a <<b' | wc -l\nbody\na <<b", "1\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// Every single-line keyword-block position: the arm body, the loop body, the
/// `for`/`case` bodies. The terminator line travels inside the arm text
/// (`cond/part.rs::reconstruct`), so the arm's operator count must be blind to
/// it and the arm's own closer stays visible.
#[test]
fn a_terminator_line_in_a_single_line_block_is_data() {
    for (script, want) in [
        ("if true; then cat <<'a <<b'; fi\nbody\na <<b", "body\n"),
        (
            "while :; do cat <<'a <<b'; break; done\nbody\na <<b",
            "body\n",
        ),
        ("for x in a; do cat <<'a <<b'; done\nbody\na <<b", "body\n"),
        ("case x in x) cat <<'a <<b';; esac\nbody\na <<b", "body\n"),
        (
            "if false; then :; elif cat <<'a <<b'; then echo T; fi\nbody\na <<b",
            "body\nT\n",
        ),
        (
            "if false; then :; else cat <<'a <<b'; fi\nbody\na <<b",
            "body\n",
        ),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// A `<<` in a block **condition**: the condition's own operator claims its
/// region, the terminator line is condition data, and the arm runs.
#[test]
fn a_terminator_line_of_a_block_condition_keeps_its_arm() {
    let (out, err, code) = run("if cat <<'a <<b'; then echo T; fi\nbody\na <<b");
    assert_eq!((code, out.as_str()), (0, "body\nT\n"), "stderr={err:?}");
    let (out, err, code) = run("while cat <<'a <<b'; do break; done\nbody\na <<b");
    assert_eq!((code, out.as_str()), (0, "body\n"), "stderr={err:?}");
    assert_eq!(err, "");
}

/// The statement after a terminator is a separate statement: the terminator's
/// newline keeps its `;` token, so a `&&` tail runs, the next here-doc claims
/// its own body, and two operators on one line each get theirs.
#[test]
fn a_terminator_line_after_a_block_is_a_separate_statement() {
    let (out, err, code) = run("cat <<'a <<b' && echo after\nbody\na <<b");
    assert_eq!((code, out.as_str()), (0, "body\nafter\n"), "stderr={err:?}");
    let (out, err, code) = run("cat <<'a <<b'\nbody\na <<b\ncat <<'c <<d'\nbody2\nc <<d");
    assert_eq!((code, out.as_str()), (0, "body\nbody2\n"), "stderr={err:?}");
    let (out, err, code) = run("cat <<'a <<b'\nbody\na <<b\necho x <<'e <<f'\nbody2\ne <<f");
    assert_eq!((code, out.as_str()), (0, "body\nx\n"), "stderr={err:?}");
    assert_eq!(err, "");
}

/// Two operators on one command line, one of them chevron-delimited, in both
/// orders: the second here-doc is the command's stdin, and the terminator lines
/// of both are data. `cat -` (dash form) and the tab-indented `<<-` terminator
/// of a chevron delimiter are the same rule.
#[test]
fn two_operators_of_which_one_is_a_chevron_delimiter_claim_their_bodies() {
    for (script, want) in [
        ("cat <<EOF <<'a <<b'\nbodyA\nbodyB\nEOF\na <<b", ""),
        ("cat <<'a <<b' <<EOF\nbodyA\nbodyB\na <<b\nEOF", ""),
        ("cat <<-'a <<b'\n\tbody\n\ta <<b", "body\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// A multi-line group body, a function body, a command substitution, and a
/// **single-line** group: in every one of them the terminator line is heredoc
/// structure, not a statement. The single-line group's row is task **#315**'s
/// flip — the group body claims its region (append mode), so the body command
/// prints and the delimiter line `a <<b` never reaches the parser as an operator.
#[test]
fn a_terminator_line_in_a_group_a_function_and_a_substitution_is_data() {
    for (script, want) in [
        ("{ cat <<'a <<b'\nbody\na <<b\n}\n", "body\n"),
        ("f(){ cat <<'a <<b'\nbody\na <<b\n}\nf\n", "body\n"),
        ("echo $(cat <<'a <<b'\nbody\na <<b\n)\n", "body\n"),
        ("{ cat <<'a <<b'; }\nbody\na <<b", "body\n"),
    ] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, want), "stderr={err:?}");
        assert_eq!(err, "");
    }
    // `( … )` is not a subshell: #316.
    let (out, err, code) = run("( cat <<'a <<b' )\nbody\na <<b");
    assert_eq!((code, out.as_str()), (1, ""), "stderr={err:?}");
    assert!(err.contains("\"(\" not found"), "stderr={err}");
}

/// The `%` capture marker and `~` on a terminator line are already bash parity
/// (the tokenizer's `%` is a capture marker, `~` a tilde trigger: both are
/// syntax bytes blanked with the rest of the terminator's syntax). Kept as
/// regression pins for the byte set of `tokenize_statement::is_structural`.
#[test]
fn a_terminator_line_with_a_capture_marker_or_tilde_is_data() {
    for script in ["cat <<'a%b'\nbody\na%b", "cat <<'a~b'\nbody\na~b"] {
        let (out, err, code) = run(script);
        assert_eq!((code, out.as_str()), (0, "body\n"), "stderr={err:?}");
        assert_eq!(err, "");
    }
}

/// Divergences of the same family that this rule does **not** own, pinned at
/// their measured rc 1 with the task that owns each one: a delimiter word cut at
/// escaped whitespace (**#349**), and an io-number here-doc whose fd a sibling
/// `<&3` of the same command cannot see (**#363**, filed from task #266's
/// planning round; measured identical on `master`).
#[test]
fn chevron_terminator_known_divergences() {
    // #349: the delimiter word ends at the escaped space, so the terminator
    // line `a b` never matches. bash prints the body (rc 0).
    let (out, err, code) = run("cat <<a\\ b\nbody\na b");
    assert_eq!((code, out.as_str()), (1, ""), "stderr={err:?}");
    assert!(
        err.contains("missing terminating delimiter line 'a b'"),
        "stderr={err}"
    );

    // #363: `<&3` takes its source from the parent's fd table, so the fd the
    // io-number here-doc opens in the same command is invisible to it. Run
    // through `run_fd3_closed` so fd 3 is closed in the parent (LESSONS: an
    // inherited fd 3 makes the assertion depend on the harness). bash prints
    // the body (rc 0).
    let (out, err, code) = run_fd3_closed("cat 3<<'a <<b' <&3\nbody\na <<b");
    assert_eq!((code, out.as_str()), (1, ""), "stderr={err:?}");
    assert!(err.contains("fd 3 is not open"), "stderr={err}");
}

/// The here-doc **delimiter word** is a quote-removal surface, never a
/// substitution surface (task #278's scope decision, measured in bash 5.3.9):
/// `cat <<$(echo EOF)` delimits on the literal `$(echo EOF)`, exactly the word
/// as written. The `<<<` word beside it **is** a substitution surface
/// (`cat <<< $(echo hi)` prints `hi`), so the two neighbouring surfaces must
/// not be "unified" — the target expansion of #278 leaves both as measured.
///
/// fdshell's terminator-line rule (#266: the terminator's syntax bytes are
/// blanked for the tokenizer) reads `$(echo EOF)` as `EOF`, so a delimiter word
/// that carries a substitution region can never be terminated: both rows below
/// are the divergence task **#372** owns, pinned at their measured rc.
#[test]
fn substitution_delimiter_word_is_literal() {
    // bash: the delimiter is the literal `$(echo EOF)`, the body is `body` and
    // `EOF`, and the warning says `wanted $(echo EOF)` (rc 0). fdshell: the
    // delimiter word is the same literal, so the `EOF` line never matches.
    let (out, err, code) = run("cat <<$(echo EOF)\nbody\nEOF\n");
    assert_eq!(
        (code, out.as_str()),
        (1, ""),
        "bash prints `body` + `EOF` at rc 0; stderr={err:?}"
    );
    assert!(
        err.contains("missing terminating delimiter line '$(echo EOF)'"),
        "the delimiter word is literal, stderr={err}"
    );
    // bash matches the literal terminator line and prints the body (rc 0);
    // fdshell's blanked terminator reads `EOF` → #372.
    let (out, err, code) = run("cat <<$(echo EOF)\nbody\n$(echo EOF)\n");
    assert_eq!(
        (code, out.as_str()),
        (1, ""),
        "bash prints `body` at rc 0 (#372); stderr={err:?}"
    );
    assert!(
        err.contains("missing terminating delimiter line"),
        "stderr={err}"
    );
    // The neighbouring surface that *is* a substitution surface, unchanged.
    let (out, err, code) = run("cat <<< $(echo hi)");
    assert_eq!((code, out.as_str()), (0, "hi\n"), "stderr={err:?}");
    // A quoted delimiter keeps its body literal, region bytes included.
    let (out, err, code) = run("cat <<'$(echo EOF)'\nbody\n$(echo EOF)\n");
    assert_eq!((code, out.as_str()), (0, "body\n"), "stderr={err:?}");
}

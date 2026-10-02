#![allow(clippy::unwrap_used)]

use super::is_complete;

#[test]
fn complete_simple_line() {
    assert!(is_complete(b"echo hi"));
    assert!(is_complete(b"echo hi   "));
}

#[test]
fn if_block_open_and_closed() {
    assert!(!is_complete(b"if true; then"));
    assert!(is_complete(b"if true; then\nfi"));
}

#[test]
fn while_block_open_and_closed() {
    assert!(!is_complete(b"while true; do"));
    assert!(is_complete(b"while true; do\necho hi\ndone"));
}

#[test]
fn case_block_open_and_closed() {
    assert!(!is_complete(b"case x in"));
    assert!(is_complete(b"case x in\n*) ;;\nesac"));
}

#[test]
fn function_block_open_and_closed() {
    assert!(!is_complete(b"f() {"));
    assert!(is_complete(b"f() {\necho hi\n}"));
}

#[test]
fn nested_if_needs_matching_fi() {
    assert!(!is_complete(b"if a; then\nif b; then\nfi"));
    assert!(is_complete(b"if a; then\nif b; then\nfi\nfi"));
}

#[test]
fn heredoc_open_and_closed() {
    assert!(!is_complete(b"cat <<EOF"));
    assert!(is_complete(b"cat <<EOF\nhello\nEOF"));
}

#[test]
fn heredoc_empty_quoted_delimiter_completes_on_blank_line() {
    assert!(!is_complete(b"cat <<\"\""));
    assert!(is_complete(b"cat <<\"\"\n\n"));
}

#[test]
fn heredoc_bare_chevron_is_hard_error_not_continuation() {
    assert!(is_complete(b"cat <<"));
}

#[test]
fn heredoc_semicolon_terminated_continues_for_body() {
    // A `;` after the operator: the body is read after the whole logical
    // line, so the REPL buffers until the delimiter line arrives.
    assert!(!is_complete(b"cat <<EOF ;"));
    assert!(is_complete(b"cat <<EOF ;\nbody\nEOF"));
}

#[test]
fn heredoc_body_with_unbalanced_quote_is_complete() {
    // The body is opaque: an unbalanced quote in it does not continue.
    assert!(is_complete(b"cat <<EOF\n\"q\nEOF"));
}

#[test]
fn trailing_cond_list_operator_opens() {
    assert!(!is_complete(b"echo a &&"));
    assert!(is_complete(b"echo a &&\necho b"));
    assert!(!is_complete(b"false ||"));
    assert!(is_complete(b"false ||\necho b"));
}

#[test]
fn trailing_pipe_is_hard_error_not_continuation() {
    // The pipeline parser cannot span a `|` across a newline.
    assert!(is_complete(b"echo a |"));
}

#[test]
fn lone_amp_and_pipe_with_trailing_space_are_complete() {
    // A lone `&` or `|` followed by whitespace is not a two-char trailing
    // operator (no backgrounding; the pipeline parser cannot span a `|`):
    // it is a hard parse error, not a continuation. Pins `top_level_op`'s
    // `&`/`|` guards and the 2-char lengths.
    assert!(is_complete(b"echo a & "));
    assert!(is_complete(b"echo a | "));
}

#[test]
fn trailing_semicolon_is_complete() {
    assert!(is_complete(b"echo hi;"));
}

#[test]
fn quoted_cond_list_operator_is_complete() {
    assert!(is_complete(b"echo \"a && b\""));
}

#[test]
fn comment_with_cond_list_operator_is_complete() {
    assert!(is_complete(b"# comment &&"));
}

#[test]
fn unbalanced_quote_opens() {
    assert!(!is_complete(b"echo \"abc"));
}

#[test]
fn unbalanced_substitution_opens() {
    assert!(!is_complete(b"echo $(true"));
}

#[test]
fn unbalanced_backtick_opens() {
    assert!(!is_complete(b"echo `abc"));
}

#[test]
fn balanced_substitution_across_lines_is_complete() {
    assert!(is_complete(b"echo $(true\n)"));
}

#[test]
fn empty_line_is_complete() {
    assert!(is_complete(b""));
}

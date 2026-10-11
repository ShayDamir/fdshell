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
fn wait_builtin_statement_is_complete() {
    assert!(is_complete(b"wait"));
    assert!(is_complete(b"wait 123"));
    assert!(is_complete(b"wait \"readable\""));
}

#[test]
fn wait_block_open_and_closed() {
    assert!(!is_complete(b"wait\n readable %rd) echo hi ;;"));
    assert!(is_complete(b"wait\n readable %rd) echo hi ;;\ndone"));
}

#[test]
fn function_block_open_and_closed() {
    assert!(!is_complete(b"f() {"));
    assert!(is_complete(b"f() {\necho hi\n}"));
}

/// The `function name { … }` keyword form continues on an open brace.
#[test]
fn function_keyword_block_open_and_closed() {
    assert!(!is_complete(b"function f {"));
    assert!(!is_complete(b"function f() {"));
    assert!(is_complete(b"function f { echo hi; }"));
    assert!(is_complete(b"function f ( ) { echo hi; }"));
    assert!(!is_complete(b"function f"));
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

/// A `{ …; }` group: an unclosed group keeps the REPL reading lines; a closed
/// one (with its trailing redirection, `&&`, or here-doc body) is complete.
#[test]
fn group_open_and_closed() {
    assert!(!is_complete(b"{ echo a"));
    assert!(!is_complete(b"{ cat <<EOF"));
    assert!(!is_complete(b"{ echo a; } &&"));
    assert!(is_complete(b"{ echo a; }"));
    assert!(is_complete(b"{ echo a\n}"));
    assert!(is_complete(b"{ echo a; } > out"));
    assert!(is_complete(b"{ cat <<EOF\nhi\nEOF\n; }"));
    assert!(is_complete(b"{ { echo n; }; }"));
    assert!(!is_complete(b"{ { echo n; }"));
}

/// A group inside a keyword block nets out of the depth: the `if` still needs
/// its `fi`, and the braces do not close it early.
#[test]
fn group_inside_block_needs_its_closing_keyword() {
    assert!(!is_complete(b"if true; then { echo a; }"));
    assert!(is_complete(b"if true; then { echo a; }; fi"));
    assert!(!is_complete(b"if true; then { echo a; };"));
}

// Task #130: a quoted `$( … )`, `${ … }` or backtick region is one unit, so the
// separators inside it (`;`, `|`, `&&`, `<`/`>`) neither split the line nor make
// it look incomplete — the buffer is executed as written, with no continuation.
#[test]
fn quoted_regions_with_inner_separators_are_complete() {
    for line in [
        b"echo \"$(printf \"a; b\")\"".as_slice(),
        b"echo \"`printf \"a; b\"`\"".as_slice(),
        b"echo \"${u:-\"a; b\"}\"".as_slice(),
        b"echo \"$(printf \"a && b\")\" && echo ok".as_slice(),
        b"echo \"$(printf \"a | b\")\" | cat".as_slice(),
        b"echo \"$(printf \"<<EOF\")\"".as_slice(),
        b"echo \"$(echo \"$(printf \"x && y\")\")\" && echo ok".as_slice(),
        b"echo \"$(printf \"a; b\")\" > out".as_slice(),
        b"if true; then echo \"$(echo \"x; y\")\"; fi".as_slice(),
        b"while true; do echo \"$(echo \"a; b\")\"; done".as_slice(),
    ] {
        assert!(
            is_complete(line),
            "{line:?} must execute without continuation"
        );
    }
}

// The other side of the same rule: a region that never closes is incomplete,
// exactly as an unbalanced quote is, so the REPL keeps reading.
#[test]
fn unterminated_quoted_regions_need_a_continuation() {
    for line in [
        b"echo \"$(echo a".as_slice(),
        b"echo \"${u:-x".as_slice(),
        b"echo \"`a".as_slice(),
        b"echo \"$(printf \"a; b\")\" &&".as_slice(),
    ] {
        assert!(!is_complete(line), "{line:?} needs a continuation line");
    }
}

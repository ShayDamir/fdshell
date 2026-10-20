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

/// Task #322: a here-doc inside a *closed* single-line keyword block keeps
/// the prompt reading until its body lines arrive — "closed" is the block
/// text's completeness, not its body stream's (bash attaches the body and
/// prints `{a,b}`, measured against bash 5.3.9). The top-level twin is
/// `heredoc_open_and_closed`.
#[test]
fn single_line_block_heredoc_continues_for_body() {
    for open in [
        b"for x in a; do cat <<T; done".as_slice(),
        b"if true; then cat <<T; fi".as_slice(),
        b"while :; do cat <<T; break; done".as_slice(),
        b"until false; do cat <<T; break; done".as_slice(),
        b"case x in x) cat <<T;; esac".as_slice(),
        b"if false; then :; else cat <<T; fi".as_slice(),
        b"{ cat <<T; }".as_slice(),
    ] {
        assert!(
            !is_complete(open),
            "{open:?}: the body line has not arrived"
        );
        assert!(
            is_complete(&[open, b"\n{a,b}\nT".as_slice()].concat()),
            "{open:?}: complete once the body arrives"
        );
    }
}

/// The `elif` arm site: the `E` delimiter keeps the prompt reading, and its
/// arrival completes the buffer.
#[test]
fn elif_block_heredoc_continues_for_body() {
    assert!(!is_complete(
        b"if false; then :; elif true; then cat <<E; fi"
    ));
    assert!(is_complete(
        b"if false; then :; elif true; then cat <<E; fi\nbodyE\nE"
    ));
}

/// A closed block plus a trailing statement's here-doc on the same line: the
/// block arm claims its own region head-first, the trailing statement's arm
/// covers the tail, so the line completes only after both bodies arrive.
#[test]
fn closed_block_with_trailing_heredoc_needs_both_bodies() {
    assert!(!is_complete(b"if true; then cat <<T; fi; cat <<A"));
    assert!(!is_complete(
        b"if true; then cat <<T; fi; cat <<A\nbodyT\nT"
    ));
    assert!(is_complete(
        b"if true; then cat <<T; fi; cat <<A\nbodyT\nT\nbodyA\nA"
    ));
}

/// A bare `<<` in a closed block is a hard parse error, not a continuation,
/// mirroring the top-level `heredoc_bare_chevron_is_hard_error_not_continuation`.
#[test]
fn bare_chevron_in_closed_block_is_complete() {
    assert!(is_complete(b"if true; then cat <<; fi"));
}

/// A here-doc at a closed block's *condition* position reads its body at the
/// prompt (bash reads it; the script-mode condition-body execution is #138's
/// rule, pinned in `tests/heredoc.rs`).
#[test]
fn condition_position_block_heredoc_continues_for_body() {
    assert!(!is_complete(b"if cat <<T; then echo x; fi"));
    assert!(is_complete(b"if cat <<T; then echo x; fi\nbodyT\nT"));
}

/// With its body arrived the closed block stays complete, and a trailing
/// statement without a here-doc adds nothing to the check.
#[test]
fn closed_block_with_arrived_body_and_trailing_statement_is_complete() {
    assert!(is_complete(
        b"if true; then cat <<T; fi; echo after\nbodyT\nT"
    ));
}

/// A `<<` word that *begins* a command line is a command word, not an operator
/// (script-mode pin
/// `heredoc_after_semicolon_with_no_command_word_consumes_its_body`), so the
/// closed-block arm's window must never reach it: the line is complete. Pins
/// `cmd_line_end`'s bound (kills the `||`→`&&` mutant).
#[test]
fn statement_initial_chevron_after_a_separator_is_complete() {
    assert!(is_complete(b"echo x; <<T"));
    assert!(is_complete(b"if true; then :; fi; <<T"));
    assert!(is_complete(b"for x in a; do :; done; <<T"));
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

/// Task #207: a quoted delimiter word with a space (`<<'a b'`, `<<"a b"`) is
/// one word whose terminator line is the stripped word, so the REPL keeps
/// reading until that line arrives.
#[test]
fn heredoc_quoted_delimiter_with_a_space_continues_to_its_line() {
    for open in [b"cat <<'a b'" as &[u8], b"cat <<\"a b\"", b"cat <<a'b'c"] {
        assert!(!is_complete(open), "the delimiter line has not arrived");
    }
    assert!(is_complete(b"cat <<'a b'\nbody\na b"));
    assert!(is_complete(b"cat <<\"a b\"\nbody\na b"));
    assert!(is_complete(b"cat <<a'b'c\nbody\nabc"));
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

/// A `'…'` span behaves like a `"…"` region for the REPL (task #141): its
/// separators, operators and newlines are data, and a closed span on a
/// statement that ends properly is complete.
#[test]
fn single_quoted_regions_with_inner_separators_are_complete() {
    for line in [
        b"echo 'a; b'".as_slice(),
        b"echo 'a && b' && echo ok".as_slice(),
        b"echo 'a | b' | cat".as_slice(),
        b"echo '<<EOF'".as_slice(),
        b"if true; then echo 'x; y'; fi".as_slice(),
        b"while true; do echo 'a; b'; done".as_slice(),
        b"echo 'a' 'b'".as_slice(),
        // A span's quote bytes are dropped, so the empty span is a word too.
        b"echo ''".as_slice(),
    ] {
        assert!(
            is_complete(line),
            "{line:?} must execute without continuation"
        );
    }
}

/// An unpaired `'` keeps the REPL reading; a statement that ends in a dangling
/// operator behind a closed span still needs its continuation line.
#[test]
fn unpaired_span_and_dangling_operator_need_continuation() {
    for line in [
        b"echo '".as_slice(),
        b"echo 'a".as_slice(),
        b"echo 'a' '".as_slice(),
        b"echo 'a' &&".as_slice(),
        b"echo 'a' && ".as_slice(),
        b"if true; then echo 'a".as_slice(),
    ] {
        assert!(!is_complete(line), "{line:?} needs a continuation line");
    }
}

/// A plain `&` inside a block's text (task #354) leaves the **line-completeness**
/// decision alone: a shape with its closer is complete (the REPL runs it), an
/// unclosed one is the `> ` continuation buffer, and the statement-level `&` is
/// unchanged. The scanner gate is what makes the closed shapes complete again —
/// before it, `case x in x) echo a & ;; esac` was an incomplete line answered
/// with the statement split's parse error.
#[test]
fn ampersand_inside_block_text_keeps_the_completeness_rule() {
    for line in [
        b"case x in x) echo a & ;; esac".as_slice(),
        b"case x in x) echo a & ;;esac".as_slice(),
        b"case x in x) echo a & ;;\nesac".as_slice(),
        b"for i in 1; do echo hi & done".as_slice(),
        b"while :; do echo hi & break; done".as_slice(),
        b"{ echo hi & }".as_slice(),
        b"f(){ echo hi & }".as_slice(),
        b"if true & then echo x; fi".as_slice(),
        b"if true; then echo x & fi".as_slice(),
        b"echo hi &".as_slice(),
    ] {
        assert!(is_complete(line), "{line:?} must be complete");
    }
    // The closer has not arrived: the buffer keeps reading.
    // (the `;;`-behind-the-`&` shape with no `esac` is one of them: the span
    // stays open, and the buffer reads on — the script form of that line is a
    // parse error, bash's rc 2.)
    for line in [
        b"case x in x) echo a & ;;".as_slice(),
        b"case x in x) echo a & ;; echo b".as_slice(),
        b"if x & echo y".as_slice(),
        b"for i in 1 & echo x".as_slice(),
        b"f(){ echo a &".as_slice(),
    ] {
        assert!(!is_complete(line), "{line:?} must be incomplete");
    }
}

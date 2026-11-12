#![allow(clippy::unwrap_used)]

use crate::scan::Quotes;

/// Brace-scan a `name() { … }` line and return its exclusive end + closed flag.
fn scan(line: &[u8]) -> Option<(usize, bool)> {
    crate::brace::scan_function_block(line, line, 0, Quotes::new())
}

#[test]
fn closes_at_matching_brace() {
    let line = b"f() { x=1; }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn nested_brace_in_param_expansion_balances() {
    // `${a:-b}` adds a balanced inner brace pair; the final `}` still closes.
    let line = b"f() { x=${a:-b}; }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn quoted_braces_are_not_counted() {
    let close = b"f() { v=\"}\"; }";
    assert_eq!(scan(close), Some((close.len(), true)));
    let open = b"f() { v=\"{\"; }";
    assert_eq!(scan(open), Some((open.len(), true)));
}

#[test]
fn brace_inside_command_subst_is_not_counted() {
    let line = b"f() { v=$(echo {); }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn brace_inside_backticks_is_not_counted() {
    let line = b"f() { v=`echo {`; }";
    assert_eq!(scan(line), Some((line.len(), true)));
}

#[test]
fn comment_runs_to_end_and_leaves_block_unclosed() {
    // A `#` comment runs to end of line, so a `}` after it is unreachable.
    let r = scan(b"f() { x=1 # c");
    assert!(r.is_some_and(|(_, closed)| !closed));
}

/// A comment row **in front of** the body's `{` is opaque to the opener search (task
/// **#518**): a brace written inside comment text is not the body's opener, so the
/// opener is the real group behind it and the block closes — bash runs
/// `f()` + `# c {` + `{ echo a; }` + `f` as `a` at rc 0 (measured). The two rows below
/// differ only in whether the comment's text carries a brace, and that pair is the
/// detector: the plain comment row already closed here on the naive byte search, the
/// one with a brace in its text came back open. The arithmetic-`for` twin
/// `)) ;` + `# c {` + `{ echo a; }` reaches this same helper (pinned at
/// `tests/loops.rs::c_style_for_every_spelling_of_a_gap_comment_matches_bash`).
#[test]
fn a_comment_row_hides_a_brace_written_in_its_text() {
    let plain = b"f()\n# c\n{ echo a; }";
    assert_eq!(scan(plain), Some((plain.len(), true)));
    let brace_in_comment = b"f()\n# c {\n{ echo a; }";
    assert_eq!(scan(brace_in_comment), Some((brace_in_comment.len(), true)));
}

#[test]
fn non_function_is_rejected() {
    assert_eq!(scan(b"plain command"), None);
}

// A `}`-shaped heredoc body line must not close the block; only the real
// `}` does.
#[test]
fn heredoc_body_brace_does_not_close() {
    let line = b"f() { cat <<EOF\n}\nEOF\n}";
    assert_eq!(scan(line), Some((line.len(), true)));
}

/// The `function name [()] {` keyword forms: the span is the definer text to the
/// closing `}`, exactly as the POSIX form's.
#[test]
fn keyword_form_spans_close_at_brace() {
    for line in [
        &b"function f { echo hi; }"[..],
        &b"function f() { echo hi; }"[..],
        &b"function f () { echo hi; }"[..],
        &b"function f ( ) { echo hi; }"[..],
    ] {
        assert_eq!(scan(line), Some((line.len(), true)), "{line:?}");
    }
}

/// `function f` with no `{` on the line: the span search finds no brace, so the
/// block is claimed open (`false`), as the POSIX `f()` form does.
#[test]
fn keyword_form_without_a_brace_is_open() {
    assert_eq!(scan(&b"function f"[..]), Some((10, false)));
    let next_line = &b"function f\n{ echo hi; }"[..];
    assert_eq!(scan(next_line), Some((next_line.len(), true)));
}

#[test]
fn keyword_form_without_a_definer_name_is_rejected() {
    assert_eq!(scan(&b"function f(x) { echo hi; }"[..]), None);
    assert_eq!(scan(&b"function { echo hi; }"[..]), None);
    assert_eq!(scan(&b"function"[..]), None);
    assert_eq!(scan(&b"function ) f { echo hi; }"[..]), None);
}

/// A `)` glued to `function` is its own word (the tokenizer's separator token),
/// so the definer pair is `function` + `)`, which is not a name: no span is
/// claimed, and the text is not claimed by a def the token layer rejects.
#[test]
fn glued_close_paren_after_the_keyword_is_not_a_block() {
    assert_eq!(scan(&b"function)f g { echo hi; }"[..]), None);
    assert_eq!(scan(&b"function)f { echo hi; }"[..]), None);
    // A `)` after the name word ends the definer pair; the span is unchanged.
    let glued_name = &b"function f)g { echo hi; }"[..];
    assert_eq!(scan(glued_name), Some((glued_name.len(), true)));
}

// A nested `{`-shaped body line must not open a brace depth level.
#[test]
fn heredoc_body_open_brace_does_not_open_depth() {
    let line = b"f() { cat <<EOF\n{\nEOF\n}";
    assert_eq!(scan(line), Some((line.len(), true)));
}

/// A here-doc **operator word is one unit** — its bytes *and* its operand word —
/// so a brace-shaped operand moves no depth: a `}` operand does not close the
/// block (the row that used to end it at the `}` inside `<<}`) and a `{` operand
/// does not open one (the pair that differs only in the operand's shape is the
/// detector). Glued spellings here, the bare spellings one test below.
#[test]
fn a_glued_operator_word_moves_no_brace_depth() {
    let close = b"f() { cat <<}; echo x; }";
    assert_eq!(scan(close), Some((close.len(), true)));
    let open = b"f() { cat <<{; echo x; }";
    assert_eq!(scan(open), Some((open.len(), true)));
}

/// The same rule at the bare spelling (`<< }`, `<< {`): the operand word behind the
/// operator word is inside the same unit.
#[test]
fn a_bare_operator_word_moves_no_brace_depth() {
    let close = b"f() { cat << }; echo x; }";
    assert_eq!(scan(close), Some((close.len(), true)));
    let open = b"f() { cat << {; echo x; }";
    assert_eq!(scan(open), Some((open.len(), true)));
}

/// A **quoted** operand is word data, so the helper hands it back to the scan's own
/// quote state: the `}` inside a closed quote span is inert, and an operand that
/// opens a quote runs it to the end of the line — the brace behind it is inert too,
/// which is what keeps the accepted divergence
/// `tests/group.rs::literal_brace_argument_in_a_block_body` alive at this
/// granularity (the walk's twin of the same control is
/// `parse/semi/closing/tests.rs::a_heredoc_structure_word_never_scores`).
#[test]
fn a_quoted_operand_stays_with_the_quote_state() {
    let closed = b"f() { cat <<\"}\"; echo x; }";
    assert_eq!(scan(closed), Some((closed.len(), true)));
    let open = b"f() { cat <<{\"; echo }; }";
    assert_eq!(scan(open), Some((open.len(), false)));
}

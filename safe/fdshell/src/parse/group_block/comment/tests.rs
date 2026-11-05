#![allow(clippy::unwrap_used)]
use alloc::string::ToString;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn text(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn group(b: &[u8]) -> crate::parse::group_block::GroupBlock {
    let tokens = crate::parse::token::tokenize_statement(b).unwrap();
    crate::parse::group_block::tokens_to_group(&tokens, &text(b)).unwrap()
}

/// The parse error's display text, as the user sees it.
fn group_err(b: &[u8]) -> alloc::string::String {
    match crate::parse::parse(&text(b)) {
        Ok(_) => panic!("expected a parse error for {b:?}"),
        Err(e) => e.current_context().to_string(),
    }
}

/// A `#` comment closing the body's last line is the brace group's terminator:
/// the comment's own words are **not** run, so the body stays the words before the
/// `#`. A brace named inside the comment is comment text — it neither opens a group
/// nor closes one — and the arm accepts the closer written on the next line anyway.
#[test]
fn a_comment_line_terminates_the_body() {
    for (script, body) in [
        (b"{ echo a # c\n}".as_slice(), "echo a"),
        (b"{ echo a # {\n}".as_slice(), "echo a"),
        (b"{ echo a # }\n}".as_slice(), "echo a"),
    ] {
        assert!(
            group(script).body.data.eq_bytes(body.as_bytes()),
            "{script:?}"
        );
    }
}

/// The comment rule applies to a cond list inside the body: the `&&` written in the
/// comment never runs, the body ends at the word before the `#`.
#[test]
fn a_comment_line_terminates_a_cond_list_body() {
    assert!(
        group(b"{ echo a && echo b # {\n}")
            .body
            .data
            .eq_bytes(b"echo a && echo b")
    );
}

/// The word-start half of the rule: a `#` inside a quoted word is word data, so the
/// comment here starts at the `#` behind the quoted word, and the quoted word stays in
/// the body with its quotes (the body is verbatim text, re-parsed as a script).
#[test]
fn a_comment_behind_a_quoted_word_terminates_the_body() {
    assert!(
        group(b"{ echo a \"b # c\" # d\n}")
            .body
            .data
            .eq_bytes(b"echo a \"b # c\"")
    );
}

/// The negative control, from the other side of the gap: the bytes between the body's
/// last word and the closer hold no comment, so the `;`-before-`}` rule still reports.
#[test]
fn a_gap_without_a_comment_is_still_an_error() {
    assert_eq!(group_err(b"{ echo a }"), "expected ';' before '}'");
    assert_eq!(group_err(b"{  2> out }"), "expected ';' before '}'");
}

/// The closer's redirections still belong to the group when the body's last line is a
/// comment-terminated one — the parse arm's answer is the predecessor test alone.
#[test]
fn a_comment_line_keeps_the_closers_redirect() {
    let g = group(b"{ echo a # c\n} >out");
    assert!(g.body.data.eq_bytes(b"echo a"));
    assert_eq!(g.redirects.len(), 1);
}

/// The two halves of the arm's predicate agree (task #425): the compound closer here
/// already answers through the shared set, and the comment keeps the body the words
/// before the `#` — the comment's own text stays out of the body, as everywhere else
/// in this file.
#[test]
fn a_compound_closer_and_a_comment_line_agree() {
    let g = group(b"{ if :; then :; fi # c\n}");
    assert!(g.body.data.eq_bytes(b"if :; then :; fi"));
}

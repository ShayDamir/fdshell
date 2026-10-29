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

fn cstr(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn group(b: &[u8]) -> crate::parse::group_block::GroupBlock {
    let tokens = crate::parse::token::tokenize_statement(b).unwrap();
    crate::parse::group_block::tokens_to_group(&tokens, &text(b)).unwrap()
}

fn parse_group(b: &[u8]) -> crate::parse::ParsedLine {
    crate::parse::parse(&text(b)).unwrap()
}

/// The parse error's display text, as the user sees it.
fn group_err(b: &[u8]) -> alloc::string::String {
    match crate::parse::parse(&text(b)) {
        Ok(_) => panic!("expected a parse error for {b:?}"),
        Err(e) => e.current_context().to_string(),
    }
}

/// The body is the text of the `compound_list` between the braces: the bracketing
/// `;`s are trimmed (`semi::trim_body`, the block-arm rule task #305 pins for the
/// definer), the words in between stay verbatim.
#[test]
fn body_span_is_byte_exact() {
    let g = group(b"{ echo a; }");
    assert!(g.body.data.eq_bytes(b"echo a"));
    assert!(g.redirects.is_empty());
    let g = group(b"  {  x=1; echo $x ;  }  ");
    assert!(g.body.data.eq_bytes(b"x=1; echo $x"));
    // Only the bracketing `;`s go: the `;` between two body statements stays.
    let g = group(b"{ echo a; echo b; }");
    assert!(g.body.data.eq_bytes(b"echo a; echo b"));
}

/// A newline before `}` is the `;` token the tokenizer emits for it, so it is the
/// bracketing terminator the trimmed body drops.
#[test]
fn newline_before_brace_is_the_semicolon() {
    let g = group(b"{ echo a\n}");
    assert!(g.body.data.eq_bytes(b"echo a"));
}

/// The closer must be preceded by a command-list terminator; the plain `&` is
/// one, and the body keeps it (task #354: the body's own scan makes the last
/// statement a background job).
#[test]
fn amp_before_brace_is_a_list_terminator() {
    let g = group(b"{ echo hi & }");
    assert!(g.body.data.eq_bytes(b"echo hi &"));
    assert!(g.redirects.is_empty());
    let g = group(b"{ echo a & echo b & }");
    assert!(g.body.data.eq_bytes(b"echo a & echo b &"));
}

#[test]
fn empty_group_is_an_error() {
    assert_eq!(group_err(b"{ }"), "group is empty");
    assert_eq!(group_err(b"{ ; ; }"), "group is empty");
    assert_eq!(group_err(b"{  2> out }"), "expected ';' before '}'");
    // The emptiness test is `semi::block_body`'s, run on the **trimmed** slice —
    // the block-arm helper every block body shares (task #315 moved the group to
    // it): a body of only bracketing `;`s is empty, and a body with one statement
    // keeps that statement.
    let g = group(b"{ ; echo a; }");
    assert!(g.body.data.eq_bytes(b"echo a"));
    let g = group(b"{ echo a ;; }");
    assert!(g.body.data.eq_bytes(b"echo a"));
}

#[test]
fn missing_semicolon_before_brace_is_an_error() {
    assert_eq!(group_err(b"{ echo a }"), "expected ';' before '}'");
}

#[test]
fn missing_closing_brace_is_an_error() {
    assert_eq!(group_err(b"{ echo a;"), "group is missing a closing brace");
    assert_eq!(
        group_err(b"{ { echo a; };"),
        "group is missing a closing brace"
    );
}

/// A here-doc inside the body keeps its body lines inside the span: the text
/// between the `<<` operator and the required `;` is part of the body
/// (**extension** mode — the multi-line group's regions are interleaved with its
/// own lines, so the verbatim slice covers them and the terminator stays).
#[test]
fn body_keeps_heredoc_body_lines() {
    let g = group(b"{ cat <<EOF\nhi\nEOF\n; }");
    assert!(g.body.data.eq_bytes(b"cat <<EOF\nhi\nEOF\n;"));
    assert!(g.redirects.is_empty());
}

/// A `;`-terminated here-doc in a **single-line** body is the append mode of the
/// block-region rule (task #315, the definer's shape of task #305): the stored
/// body is the operator line's bytes **plus** the claimed region, so the text the
/// group re-runs is a contiguous heredoc. The bracketing `;` is trimmed, so the
/// bytes are `cat <<T` + the region, never `cat <<T;` + the region.
#[test]
fn appended_region_travels_with_the_stored_body() {
    let g = group(b"{ cat <<T; }\nbody\nT");
    assert!(g.body.data.eq_bytes(b"cat <<T\nbody\nT"));
    assert!(g.redirects.is_empty());
    // Two operators, in operator order.
    let g = group(b"{ cat <<T; cat <<U; }\nbT\nT\nbU\nU");
    assert!(g.body.data.eq_bytes(b"cat <<T; cat <<U\nbT\nT\nbU\nU"));
    // The text carries no body lines: the operator's region is the empty
    // end-of-file one (task #364), the tail is untouched, and the stored body is
    // the trimmed slice plus the region — a text the parse layer decides for:
    // empty body, warning, rc 0 (bash warns and runs, measured 2026-10-29).
    let g = group(b"{ cat <<T; }");
    assert!(g.body.data.eq_bytes(b"cat <<T\n"));
    assert!(g.redirects.is_empty());
}

/// The tail filter: the appended region's **terminator** word stays a token
/// (`tokenize_statement` blanks a body region's words and not a terminator's,
/// task #266), so the tail analysis must not read it as a group tail word. A
/// redirection the group's own text wrote after `}` stays, and with no appended
/// region the tail is untouched (every pinned tail message byte-ident).
#[test]
fn appended_heredoc_structure_is_not_a_tail_word() {
    // The delimiter word and the newlines around the region are cut.
    let g = group(b"{ cat <<T; }\nbody\nT");
    assert!(g.redirects.is_empty());
    // A tail redirect starts before the appended newline, so it survives.
    let g = group(b"{ cat <<T; } > out\nbody\nT");
    assert_eq!(g.redirects.len(), 1);
    // The tail forms that must keep their messages: the pipe and the `&` are
    // written **before** the region, and so is a tail `<<` operator.
    assert_eq!(group_err(b"{ cat <<T; } | cat\nbody\nT"), "unexpected pipe");
    assert_eq!(
        group_err(b"{ cat <<T; } &\nbody\nT"),
        "unexpected character in this context"
    );
    assert_eq!(
        group_err(b"{ cat <<T; } <<EOF\nbody\nT\nEOF"),
        "group: here-doc on a group is not supported"
    );
    // No region at all: the tail words are the group's own text.
    assert_eq!(
        group_err(b"{ echo a; } b"),
        "group: unexpected word after '}'"
    );
}

/// A nested group nets out of the brace depth, so the outer body is the whole
/// inner text and the closer is the outer `}`.
#[test]
fn nested_group_bodies() {
    let g = group(b"{ { echo n; }; }");
    assert!(g.body.data.eq_bytes(b"{ echo n; }"));
    let g = group(b"{ echo a; { echo b; }; }");
    assert!(g.body.data.eq_bytes(b"echo a; { echo b; }"));
}

/// `;` chains inside the body survive, and `&&` / `||` at the statement level
/// split before the group is parsed, so the group text carries no operator.
#[test]
fn parse_end_to_end_is_a_group() {
    let parsed = parse_group(b"{ x=1; echo $x; }");
    let crate::parse::ParsedLine::Group(g) = parsed else {
        panic!("expected Group");
    };
    assert!(g.body.data.eq_bytes(b"x=1; echo $x"));
    assert!(matches!(
        parse_group(b"{ echo a; } > out"),
        crate::parse::ParsedLine::Group(_)
    ));
}

/// A top-level literal brace word is not a group: `echo {` and `echo }` stay
/// commands, and `{}` / `{echo` are ordinary words.
#[test]
fn brace_words_are_not_groups() {
    assert!(!matches!(
        parse_group(b"echo {"),
        crate::parse::ParsedLine::Group(_)
    ));
    assert!(!matches!(
        parse_group(b"echo }"),
        crate::parse::ParsedLine::Group(_)
    ));
    assert!(!matches!(
        parse_group(b"{}; echo hi"),
        crate::parse::ParsedLine::Group(_)
    ));
    assert!(!matches!(
        parse_group(b"{echo hi; }"),
        crate::parse::ParsedLine::Group(_)
    ));
    // `{;` is split by the tokenizer into the words `{` and `;`, so only the
    // segment scanner's opener rule can keep it a command word (the scan-level
    // pins are in `segment/group/tests.rs` and `tests/group.rs`).
}

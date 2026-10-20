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

/// The body is the verbatim text between the braces, with the required `;`
/// kept inside the span.
#[test]
fn body_span_is_byte_exact() {
    let g = group(b"{ echo a; }");
    assert!(g.body.data.eq_bytes(b"echo a;"));
    assert!(g.redirects.is_empty());
    let g = group(b"  {  x=1; echo $x ;  }  ");
    assert!(g.body.data.eq_bytes(b"x=1; echo $x ;"));
}

/// A newline before `}` is the `;` token the tokenizer emits for it.
#[test]
fn newline_before_brace_is_the_semicolon() {
    let g = group(b"{ echo a\n}");
    assert!(g.body.data.eq_bytes(b"echo a\n"));
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
/// between the `<<` operator and the required `;` is part of the body.
#[test]
fn body_keeps_heredoc_body_lines() {
    let g = group(b"{ cat <<EOF\nhi\nEOF\n; }");
    assert!(g.body.data.eq_bytes(b"cat <<EOF\nhi\nEOF\n;"));
    assert!(g.redirects.is_empty());
}

/// A nested group nets out of the brace depth, so the outer body is the whole
/// inner text and the closer is the outer `}`.
#[test]
fn nested_group_bodies() {
    let g = group(b"{ { echo n; }; }");
    assert!(g.body.data.eq_bytes(b"{ echo n; };"));
    let g = group(b"{ echo a; { echo b; }; }");
    assert!(g.body.data.eq_bytes(b"echo a; { echo b; };"));
}

/// `;` chains inside the body survive, and `&&` / `||` at the statement level
/// split before the group is parsed, so the group text carries no operator.
#[test]
fn parse_end_to_end_is_a_group() {
    let parsed = parse_group(b"{ x=1; echo $x; }");
    let crate::parse::ParsedLine::Group(g) = parsed else {
        panic!("expected Group");
    };
    assert!(g.body.data.eq_bytes(b"x=1; echo $x;"));
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

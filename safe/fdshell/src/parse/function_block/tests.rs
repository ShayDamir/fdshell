#![allow(clippy::unwrap_used)]
use sys::{Origin, Position, ScriptText, ShortCStr};

fn text(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn parse(b: &[u8]) -> crate::parse::ParsedLine {
    crate::parse::parse(&text(b)).unwrap()
}

#[test]
fn parses_function_name_and_body() {
    let crate::parse::ParsedLine::Function(def) = parse(b"foo() { echo hi; }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"foo"));
    assert!(def.body.data.eq_bytes(b"echo hi"));
}

#[test]
fn parses_empty_body() {
    let crate::parse::ParsedLine::Function(def) = parse(b"bar() { }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"bar"));
    assert!(def.body.data.is_empty());
}

#[test]
fn body_is_verbatim_not_expanded() {
    let crate::parse::ParsedLine::Function(def) = parse(b"f() { g=$x; }") else {
        panic!("expected Function");
    };
    assert!(def.body.data.eq_bytes(b"g=$x"));
}

#[test]
fn body_keeps_nested_commands() {
    let crate::parse::ParsedLine::Function(def) = parse(b"f() { a=1; b=2; }") else {
        panic!("expected Function");
    };
    assert!(def.body.data.eq_bytes(b"a=1; b=2"));
}

#[test]
fn no_parens_is_not_a_function() {
    assert!(!matches!(
        parse(b"foo { x }"),
        crate::parse::ParsedLine::Function(_)
    ));
}

#[test]
fn empty_name_is_not_a_function() {
    assert!(!matches!(
        parse(b"() { x }"),
        crate::parse::ParsedLine::Function(_)
    ));
}

#[test]
fn paren_but_no_brace_is_not_a_function() {
    assert!(!matches!(
        parse(b"foo() x"),
        crate::parse::ParsedLine::Function(_)
    ));
}

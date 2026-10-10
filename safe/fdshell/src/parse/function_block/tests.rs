#![allow(clippy::unwrap_used)]
use crate::error::parse::ParseError;
use error_stack::Report;
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

fn try_parse(b: &[u8]) -> Result<crate::parse::ParsedLine, Report<ParseError>> {
    crate::parse::parse(&text(b))
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

/// The bash/ksh keyword form: every definer shape stores the same name and body.
#[test]
fn keyword_form_definer_shapes() {
    for text in [
        &b"function f { echo hi; }"[..],
        &b"function f() { echo hi; }"[..],
        &b"function f () { echo hi; }"[..],
        &b"function f ( ) { echo hi; }"[..],
    ] {
        let crate::parse::ParsedLine::Function(def) = parse(text) else {
            panic!("expected Function for {text:?}");
        };
        assert!(
            def.name.eq_bytes(b"f"),
            "name of {text:?} is {:?}",
            def.name
        );
        assert!(def.body.data.eq_bytes(b"echo hi"), "body of {text:?}");
    }
}

#[test]
fn keyword_form_keeps_the_name_leniency() {
    // A reserved word, an option-shaped word, and a quoted word are names.
    let crate::parse::ParsedLine::Function(def) = parse(b"function if { echo hi; }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"if"));
    let crate::parse::ParsedLine::Function(def) = parse(b"function -a f { echo hi; }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"-a"));
    // A quoted name is stored as its unquoted word (the token's word).
    let crate::parse::ParsedLine::Function(def) = parse(b"function \"f\" { echo hi; }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"f"));
}

/// The definer rule rejects a `function` word with no definer name.
#[test]
fn keyword_form_without_a_name_is_not_a_function() {
    for text in [
        &b"function { echo hi; }"[..],
        &b"function f(x) { echo hi; }"[..],
        &b"function"[..],
        &b"function f"[..],
        &b"function ) f { echo hi; }"[..],
        // The tokenizer emits `)` as its own separator token, so the definer
        // pair of `function)f` is `function` + `)`: no name, not a definition.
        &b"function)f g { echo hi; }"[..],
    ] {
        assert!(
            !matches!(parse(text), crate::parse::ParsedLine::Function(_)),
            "{text:?} is not a definition"
        );
    }
}

#[test]
fn keyword_form_missing_close_brace_is_an_error() {
    // `ParsedLine` is not `Debug`, so map the `Ok` arm to `()` before unwrap_err.
    assert!(matches!(
        try_parse(b"function f {")
            .map(|_| ())
            .unwrap_err()
            .current_context(),
        ParseError::FunctionMissingCloseBrace
    ));
}

/// The POSIX rule applied to the word `function` names the function `function`.
#[test]
fn keyword_word_as_posix_name_is_a_function() {
    let crate::parse::ParsedLine::Function(def) = parse(b"function() { echo fn; }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"function"));
    assert!(def.body.data.eq_bytes(b"echo fn"));
}

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

/// The block-body rule of the arms and the group covers a function-definition
/// body too (task #302): POSIX #2.9's `function_def` nests the group's
/// `compound_list`, and bash stops on `f() { }` with rc 2. Every spelling of a
/// body with no command is a parse error, in both definer forms.
#[test]
fn empty_body_is_a_parse_error() {
    for text in [
        &b"bar() { }"[..],
        &b"bar() {; }"[..],
        &b"bar() {\n}"[..],
        &b"bar() { ;; }"[..],
        &b"bar() { # c\n}"[..],
        &b"function bar { }"[..],
        &b"function bar() { }"[..],
    ] {
        // `ParsedLine` is not `Debug`, so map the `Ok` arm to `()` before unwrap_err.
        assert!(
            matches!(
                try_parse(text).map(|_| ()).unwrap_err().current_context(),
                ParseError::FunctionEmptyBody
            ),
            "{text:?} is not the empty-body error"
        );
    }
}

/// The non-empty guards: one command in the same shapes keeps the definition,
/// with the body slice the shared `block_body` helper gives (the rule must not
/// widen past the empty body).
#[test]
fn non_empty_bodies_stay_definitions() {
    for (text, body) in [
        (&b"bar() { :; }"[..], b":".as_slice()),
        (&b"bar() { ;; :; }"[..], b":".as_slice()),
        (&b"function bar { :; }"[..], b":".as_slice()),
        (&b"bar() {\necho hi\n}"[..], b"echo hi".as_slice()),
    ] {
        let crate::parse::ParsedLine::Function(def) = parse(text) else {
            panic!("expected a definition for {text:?}");
        };
        assert!(def.name.eq_bytes(b"bar"), "name of {text:?}");
        assert!(def.body.data.eq_bytes(body), "body of {text:?}");
    }
}

/// A body of only the plain `&` is **not** an empty body (tasks #354/#355): the
/// definition is accepted and the body's own scan is what rejects the call, so
/// the empty-body rule must not fire on it.
#[test]
fn ampersand_only_body_is_not_the_empty_body_error() {
    for text in [&b"bar() { & }"[..], &b"bar() { & ; }"[..]] {
        assert!(
            matches!(parse(text), crate::parse::ParsedLine::Function(_)),
            "{text:?} is a definition"
        );
    }
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

/// Name leniency is a *word* rule: an option-shaped word is a name, and a
/// **quoted** word is not (task #247, POSIX §2.9.1 — bash: `` 'f'': not a valid
/// identifier ``). A reserved word as the name stays the lenient case.
#[test]
fn keyword_form_keeps_the_name_leniency() {
    // A reserved word and an option-shaped word are names.
    let crate::parse::ParsedLine::Function(def) = parse(b"function if { echo hi; }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"if"));
    let crate::parse::ParsedLine::Function(def) = parse(b"function -a f { echo hi; }") else {
        panic!("expected Function");
    };
    assert!(def.name.eq_bytes(b"-a"));
    // A quoted definer name is word data: no definition (bash rc 1, the same
    // "not a valid identifier" the name lookup then reports).
    for text in [
        &b"function \"f\" { echo hi; }"[..],
        &b"function 'f'() { echo hi; }"[..],
        &b"function 'if' { echo hi; }"[..],
        &b"function f'oo' { echo hi; }"[..],
    ] {
        assert!(
            !matches!(parse(text), crate::parse::ParsedLine::Function(_)),
            "{text:?} is not a definition"
        );
    }
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

/// Task #247: every definer word is quote-gated, so a quoted `function`, a
/// quoted `name(` and a quoted body brace all leave the shape a command line
/// (bash: `` 'function' f() `` is a syntax error, `` f'oo'() `` and `` 'f'() ``
/// are `` not a valid identifier ``). The unquoted forms are unchanged.
#[test]
fn quoted_definer_words_do_not_define() {
    for text in [
        &b"'function' f() { echo hi; }"[..],
        &b"\"function\" f() { echo hi; }"[..],
        &b"''function f() { echo hi; }"[..],
        &b"f'oo'() { echo hi; }"[..],
        &b"'foo'() { echo hi; }"[..],
        &b"foo'(') { echo hi; }"[..],
    ] {
        assert!(
            !matches!(parse(text), crate::parse::ParsedLine::Function(_)),
            "{text:?} is not a definition"
        );
    }
    // The unquoted twins are definitions, with the same name and body.
    for (text, name) in [
        (&b"foo() { echo hi; }"[..], b"foo".as_slice()),
        (&b"function foo { echo hi; }"[..], b"foo".as_slice()),
    ] {
        let crate::parse::ParsedLine::Function(def) = parse(text) else {
            panic!("expected Function for {text:?}");
        };
        assert!(def.name.eq_bytes(name), "name of {text:?}");
        assert!(def.body.data.eq_bytes(b"echo hi"), "body of {text:?}");
    }
}

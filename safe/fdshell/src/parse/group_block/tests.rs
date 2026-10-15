#![allow(clippy::unwrap_used)]
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
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

/// `(fd, direction, target)` of each redirect, ignoring the tokenizer's quote
/// mask (which is not the group's business).
fn shape(redirects: &[RedirectDef]) -> Vec<(i32, RedirectDirection, Vec<u8>)> {
    redirects
        .iter()
        .map(|r| {
            let target = match &r.source {
                RedirectSource::Path { path, .. }
                | RedirectSource::Var(path)
                | RedirectSource::HereString { word: path, .. } => {
                    path.as_bytes().unwrap_or(&[]).to_vec()
                }
                RedirectSource::Dup(from) => format!("dup{from}").into_bytes(),
                RedirectSource::Close => Vec::from(b"close".as_slice()),
                RedirectSource::HereDoc { .. } => Vec::from(b"heredoc".as_slice()),
            };
            (r.export_to, r.direction, target)
        })
        .collect()
}

/// The group's own trailing redirections, in source order.
#[test]
fn redirects_after_brace() {
    assert_eq!(
        shape(&group(b"{ echo a; } > out").redirects),
        vec![(1, RedirectDirection::Write, b"out".to_vec())]
    );
    assert_eq!(
        shape(&group(b"{ echo a; } >> out").redirects),
        vec![(1, RedirectDirection::Append, b"out".to_vec())]
    );
    assert_eq!(
        shape(&group(b"{ cat; } < in").redirects),
        vec![(0, RedirectDirection::Read, b"in".to_vec())]
    );
    assert_eq!(
        shape(&group(b"{ echo a; } 2>&1").redirects),
        vec![(2, RedirectDirection::Read, b"dup1".to_vec())]
    );
    assert_eq!(
        shape(&group(b"{ echo a; } 2>%v").redirects),
        vec![(2, RedirectDirection::Write, b"v".to_vec())]
    );
    assert_eq!(
        shape(&group(b"{ echo a; } > /dev/fd/3").redirects),
        vec![(1, RedirectDirection::Read, b"dup3".to_vec())]
    );
    assert_eq!(
        shape(&group(b"{ echo a; } > f1 2> f2").redirects),
        vec![
            (1, RedirectDirection::Write, b"f1".to_vec()),
            (2, RedirectDirection::Write, b"f2".to_vec())
        ]
    );
}

/// A `>|` after `}` is the clobber direction, in the attached and bare forms.
#[test]
fn clobber_redirect_after_brace() {
    assert_eq!(
        shape(&group(b"{ echo a; } >|f").redirects),
        vec![(1, RedirectDirection::Clobber, b"f".to_vec())]
    );
}

/// The glued `}>out3` token splits at the brace, so the operator is the
/// group's redirection and not a trailing word.
#[test]
fn glued_redirect_after_brace_is_a_redirect() {
    assert_eq!(
        shape(&group(b"{ echo a; }>out3").redirects),
        vec![(1, RedirectDirection::Write, b"out3".to_vec())]
    );
    assert_eq!(
        shape(&group(b"{ echo a; }2>err").redirects),
        vec![(2, RedirectDirection::Write, b"err".to_vec())]
    );
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

/// A trailing word, a pipe, a background `&`, and a here-doc / here-string
/// operator after `}` are the pinned parse errors.
#[test]
fn trailing_operators_and_words_are_pinned_errors() {
    assert_eq!(
        group_err(b"{ echo a; } echo b"),
        "group: unexpected word after '}'"
    );
    assert_eq!(
        group_err(b"{ echo a; } }"),
        "group: unexpected word after '}'"
    );
    assert_eq!(group_err(b"{ echo a; } | cat"), "unexpected pipe");
    assert_eq!(
        group_err(b"{ echo a; } &"),
        "unexpected character in this context"
    );
    assert_eq!(
        group_err(b"{ echo a; } <<EOF"),
        "group: here-doc on a group is not supported"
    );
    assert_eq!(
        group_err(b"{ echo a; } <<<x"),
        "group: here-doc on a group is not supported"
    );
    // The glued `}|` token splits at the brace, so the pipe is the group's
    // trailing operator: the pipeline form is the pinned unsupported error.
    assert_eq!(group_err(b"{ echo a; }|cat"), "unexpected pipe");
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

/// A redirect operator with no target after `}` propagates the redirect parse
/// error through the group tail, in the bare and the fd-qualified forms.
#[test]
fn redirect_syntax_error_propagates_through_the_group_tail() {
    assert_eq!(group_err(b"{ echo a; } >"), "invalid redirect syntax");
    assert_eq!(group_err(b"{ echo a; } 2>"), "invalid redirect syntax");
    assert_eq!(group_err(b"{ echo a; } >|"), "invalid redirect syntax");
}

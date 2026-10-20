#![allow(clippy::unwrap_used)]
use super::*;
use crate::parse::group_block::split_close;
use crate::parse::semi::closing_keyword_index;
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

/// The group's tail tokens: everything the closing brace leaves behind, with the
/// glued form split off the brace (`split_close`, the closer's own rule).
fn tail_of(b: &[u8]) -> Vec<Token> {
    let tokens = tokenize_statement(b).unwrap();
    let close = closing_keyword_index(&tokens, b).unwrap();
    split_close(&tokens, close)
}

/// The tail rule on a whole group line: the tail of `b` through [`redirects`].
fn run(b: &[u8]) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    redirects(&tail_of(b), b)
}

fn tail_redirects(b: &[u8]) -> Vec<RedirectDef> {
    run(b).unwrap()
}

/// The tail rule's parse error, as the user sees it.
fn tail_err(b: &[u8]) -> alloc::string::String {
    match run(b) {
        Ok(_) => panic!("expected a parse error for {b:?}"),
        Err(e) => e.current_context().to_string(),
    }
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
        shape(&tail_redirects(b"{ echo a; } > out")),
        vec![(1, RedirectDirection::Write, b"out".to_vec())]
    );
    assert_eq!(
        shape(&tail_redirects(b"{ echo a; } >> out")),
        vec![(1, RedirectDirection::Append, b"out".to_vec())]
    );
    assert_eq!(
        shape(&tail_redirects(b"{ cat; } < in")),
        vec![(0, RedirectDirection::Read, b"in".to_vec())]
    );
    assert_eq!(
        shape(&tail_redirects(b"{ echo a; } 2>&1")),
        vec![(2, RedirectDirection::Read, b"dup1".to_vec())]
    );
    assert_eq!(
        shape(&tail_redirects(b"{ echo a; } 2>%v")),
        vec![(2, RedirectDirection::Write, b"v".to_vec())]
    );
    assert_eq!(
        shape(&tail_redirects(b"{ echo a; } > /dev/fd/3")),
        vec![(1, RedirectDirection::Read, b"dup3".to_vec())]
    );
    assert_eq!(
        shape(&tail_redirects(b"{ echo a; } > f1 2> f2")),
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
        shape(&tail_redirects(b"{ echo a; } >|f")),
        vec![(1, RedirectDirection::Clobber, b"f".to_vec())]
    );
}

/// The glued `}>out3` token splits at the brace, so the operator is the
/// group's redirection and not a trailing word.
#[test]
fn glued_redirect_after_brace_is_a_redirect() {
    assert_eq!(
        shape(&tail_redirects(b"{ echo a; }>out3")),
        vec![(1, RedirectDirection::Write, b"out3".to_vec())]
    );
    assert_eq!(
        shape(&tail_redirects(b"{ echo a; }2>err")),
        vec![(2, RedirectDirection::Write, b"err".to_vec())]
    );
}

/// A trailing word, a pipe, a background `&`, and a here-doc / here-string
/// operator after `}` are the pinned parse errors.
#[test]
fn trailing_operators_and_words_are_pinned_errors() {
    assert_eq!(
        tail_err(b"{ echo a; } echo b"),
        "group: unexpected word after '}'"
    );
    assert_eq!(
        tail_err(b"{ echo a; } }"),
        "group: unexpected word after '}'"
    );
    assert_eq!(tail_err(b"{ echo a; } | cat"), "unexpected pipe");
    assert_eq!(
        tail_err(b"{ echo a; } &"),
        "unexpected character in this context"
    );
    assert_eq!(
        tail_err(b"{ echo a; } <<EOF"),
        "group: here-doc on a group is not supported"
    );
    assert_eq!(
        tail_err(b"{ echo a; } <<<x"),
        "group: here-doc on a group is not supported"
    );
    // The glued `}|` token splits at the brace, so the pipe is the group's
    // trailing operator: the pipeline form is the pinned unsupported error.
    assert_eq!(tail_err(b"{ echo a; }|cat"), "unexpected pipe");
}

/// A redirect operator with no target after `}` propagates the redirect parse
/// error through the group tail, in the bare and the fd-qualified forms.
#[test]
fn redirect_syntax_error_propagates_through_the_group_tail() {
    assert_eq!(tail_err(b"{ echo a; } >"), "invalid redirect syntax");
    assert_eq!(tail_err(b"{ echo a; } 2>"), "invalid redirect syntax");
    assert_eq!(tail_err(b"{ echo a; } >|"), "invalid redirect syntax");
}

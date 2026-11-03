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

/// `(fd, extra_to, direction, target)` — the shape helper plus the field the
/// `&>` family exists for (task #409), so a combined def cannot pass for a plain
/// `>` one. Path and fd-variable targets only; see `the_family_at_the_closer_names_its_extra_to`.
fn family_shape(redirects: &[RedirectDef]) -> Vec<(i32, Option<i32>, RedirectDirection, Vec<u8>)> {
    redirects
        .iter()
        .map(|r| {
            let target = match &r.source {
                RedirectSource::Path { path, .. } => path.as_bytes().unwrap_or(&[]).to_vec(),
                RedirectSource::Var(path) => {
                    let mut t = b"var:".to_vec();
                    t.extend(path.as_bytes().unwrap_or(&[]));
                    t
                }
                other => panic!("not a path/var source: {other:?}"),
            };
            (r.export_to, r.extra_to, r.direction, target)
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
        vec![(1, RedirectDirection::Write, b"/dev/fd/3".to_vec())]
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

/// The tail's whole-word `|` test is [`pipe_token_span`]'s (task #261's ONE
/// pipe-token predicate, moved here by #346): only the one-`|`-byte span is the
/// tokenizer's pipe token, so a quoted `"|"` / `'|'` and an escaped `\|` are
/// trailing **words**, the same error a plain word after `}` gives, where the
/// real `|` is the pipeline operator the group cannot take. The `&` rows are the
/// deliberately ungated neighbours: their tests are prefix tests, so the bare `&`
/// and `&&` keep their pinned errors. The `&>` family left that pair when task
/// #409 taught the tail its redirect arms — only its pidvar half still answers
/// here, in this same wording (see
/// `the_family_at_the_closer_names_its_extra_to`).
#[test]
fn a_quoted_pipe_word_is_a_trailing_word() {
    assert_eq!(tail_err(b"{ echo a; } | cat"), "unexpected pipe");
    assert_eq!(
        tail_err(b"{ echo a; } \"|\""),
        "group: unexpected word after '}'",
        "the quoted pipe is a word"
    );
    assert_eq!(
        tail_err(b"{ echo a; } '|'"),
        "group: unexpected word after '}'"
    );
    assert_eq!(
        tail_err(b"{ echo a; } \\|"),
        "group: unexpected word after '}'"
    );
    assert_eq!(
        tail_err(b"{ echo a; } \"|\" >f"),
        "group: unexpected word after '}'"
    );
    // The word-behind-the-quoted-pipe twin of the script row
    // (`tests/group.rs::a_quoted_pipe_after_brace_is_a_trailing_word`): the
    // quoted `|` and its `out` word are both trailing words, and the first of
    // them is the error. bash 5.3.9 is a syntax error (rc 2) on the script.
    assert_eq!(
        tail_err(b"{ echo a; } \"|\" out"),
        "group: unexpected word after '}'"
    );
    // The unquoted prefix rows, unchanged.
    assert_eq!(
        tail_err(b"{ echo a; } &&"),
        "unexpected character in this context"
    );
    assert_eq!(
        tail_err(b"{ echo a; } &"),
        "unexpected character in this context"
    );
}

/// The `&>` family at the group's closer (task **#409**). Every row names
/// `extra_to`, because that field is what makes a combined def: the `shape()` above
/// cannot see it, so a `&>` def would pass for a plain `>` def (LESSONS "The `&>`
/// family is one operator, so one open, so one description shared by fd 1 and fd
/// 2"). The bare `&`/`&&` rows stay the pinned parse errors (owners #188/#192), and
/// the family's **pidvar** half is refused here too, in the same wording: the pidvar
/// is a simple-command position. Values measured 2026-11-03 against bash 5.3.9 in
/// `tests/group.rs::the_combined_redirect_family_on_the_group_closer`.
#[test]
fn the_family_at_the_closer_names_its_extra_to() {
    assert_eq!(
        family_shape(&tail_redirects(b"{ echo a; } &> f")),
        vec![(1, Some(2), RedirectDirection::Write, b"f".to_vec())]
    );
    assert_eq!(
        family_shape(&tail_redirects(b"{ echo a; } &>> f")),
        vec![(1, Some(2), RedirectDirection::Append, b"f".to_vec())]
    );
    // The glued closer `}&>f` splits at the brace, like the `}>out3` row above.
    assert_eq!(
        family_shape(&tail_redirects(b"{ echo a; }&>f")),
        vec![(1, Some(2), RedirectDirection::Write, b"f".to_vec())]
    );
    assert_eq!(
        family_shape(&tail_redirects(b"{ echo a; }&>>f")),
        vec![(1, Some(2), RedirectDirection::Append, b"f".to_vec())]
    );
    // The operand consumption in both orders (POSIX #2.5: the writes apply in the
    // order they were written, and each file still opens).
    assert_eq!(
        family_shape(&tail_redirects(b"{ echo a; } &> f 2> g")),
        vec![
            (1, Some(2), RedirectDirection::Write, b"f".to_vec()),
            (2, None, RedirectDirection::Write, b"g".to_vec())
        ]
    );
    assert_eq!(
        family_shape(&tail_redirects(b"{ echo a; } 2> g &> f")),
        vec![
            (2, None, RedirectDirection::Write, b"g".to_vec()),
            (1, Some(2), RedirectDirection::Write, b"f".to_vec())
        ]
    );
    // The `%var` target stays the fd-variable source: bash reads the file name `%v`,
    // and that divergence is the family's, documented in POSIX_COMPATIBILITY
    // (">`%var` and `&>%var` redirect to the descriptor").
    assert_eq!(
        family_shape(&tail_redirects(b"{ echo a; } &>%v")),
        vec![(1, Some(2), RedirectDirection::Write, b"var:v".to_vec())]
    );
    // The refusals at this position: the pidvar arms keep the message this file
    // already gives every `&` shape, and the family's own `|` and no-operand arms
    // keep the redirect error (bash: rc 2 for all of them).
    assert_eq!(
        tail_err(b"{ echo a; } &>&p"),
        "unexpected character in this context"
    );
    assert_eq!(
        tail_err(b"{ echo a; } &>|&p"),
        "unexpected character in this context"
    );
    assert_eq!(tail_err(b"{ echo a; } &>|f"), "invalid redirect syntax");
    assert_eq!(tail_err(b"{ echo a; } &>"), "invalid redirect syntax");
    assert_eq!(tail_err(b"{ echo a; } &>>"), "invalid redirect syntax");
}

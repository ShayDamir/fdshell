#![allow(clippy::unwrap_used)]
use super::*;
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

/// The block line `b` tokenized, then read as a tail: the tokens from its
/// closing keyword `kw` on, through [`redirects`]. The closer is located by the
/// token rule ([`is_closer`]), so the glued `done>f` form is reachable here
/// exactly as the byte scan hands it over once `keywords` scores the glue.
fn run(b: &[u8], kw: &[u8]) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let tokens = tokenize_statement(b).unwrap();
    let at = (0..tokens.len())
        .rfind(|&i| tokens.get(i).is_some_and(|t| is_closer(t, kw)))
        .unwrap();
    redirects(kw, tokens.get(at..).unwrap_or(&[]), b)
}

fn tail(b: &[u8]) -> Vec<RedirectDef> {
    run(b, b"done").unwrap()
}

fn tail_kw(b: &[u8], kw: &[u8]) -> Vec<RedirectDef> {
    run(b, kw).unwrap()
}

fn tail_err(b: &[u8]) -> String {
    match run(b, b"done") {
        Ok(_) => panic!("expected a parse error for {b:?}"),
        Err(e) => e.current_context().to_string(),
    }
}

/// `(fd, direction, target)` of each redirect, ignoring the tokenizer's quote
/// mask (which is not the tail's business).
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

/// Every operator shape the tail reads, in source order, asserted as the exact
/// redirect list (the cheap kill for a tail that swallows a token).
#[test]
fn redirect_shapes_after_the_closer() {
    assert_eq!(
        shape(&tail(b"while :; do :; done > f")),
        vec![(1, RedirectDirection::Write, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done >>f")),
        vec![(1, RedirectDirection::Append, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done 3> f")),
        vec![(3, RedirectDirection::Write, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done < g")),
        vec![(0, RedirectDirection::Read, b"g".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done <> g")),
        vec![(0, RedirectDirection::Rw, b"g".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done >| f")),
        vec![(1, RedirectDirection::Clobber, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done >&2")),
        vec![(1, RedirectDirection::Read, b"dup2".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done 2>&1")),
        vec![(2, RedirectDirection::Read, b"dup1".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done >&-")),
        vec![(1, RedirectDirection::Read, b"close".to_vec())]
    );
}

/// `>%var` and `/dev/fd/N` are the same rules the group's tail gives.
#[test]
fn var_and_fd_path_targets() {
    assert_eq!(
        shape(&tail(b"while :; do :; done >%v")),
        vec![(1, RedirectDirection::Write, b"v".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done > /dev/fd/1")),
        vec![(1, RedirectDirection::Read, b"dup1".to_vec())]
    );
}

/// Several redirections keep their source order: `Scope::open` sorts them
/// stably per fd, so the last one to a fd wins while each entry still
/// opens/truncates its target (bash's `done > f > g` measurement).
#[test]
fn several_redirects_stay_in_source_order() {
    assert_eq!(
        shape(&tail(b"while :; do :; done > f > g")),
        vec![
            (1, RedirectDirection::Write, b"f".to_vec()),
            (1, RedirectDirection::Write, b"g".to_vec())
        ]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done > f 2> g")),
        vec![
            (1, RedirectDirection::Write, b"f".to_vec()),
            (2, RedirectDirection::Write, b"g".to_vec())
        ]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done >&2 3> f")),
        vec![
            (1, RedirectDirection::Read, b"dup2".to_vec()),
            (3, RedirectDirection::Write, b"f".to_vec())
        ]
    );
}

/// The operand is the raw word: a command substitution is not expanded by the
/// tail (the run layer substitutes it), and a quoted operand keeps its text.
#[test]
fn the_operand_stays_raw() {
    assert_eq!(
        shape(&tail(b"while :; do :; done > $(echo f)")),
        vec![(1, RedirectDirection::Write, b"$(echo f)".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done > \"f\"")),
        vec![(1, RedirectDirection::Write, b"f".to_vec())]
    );
}

/// The glued closer: `done>f` splits at the keyword, so the operator is the
/// block's redirection. The mask and span shifts are checked through a quoted
/// operand (`done>"f"`), whose quote span starts inside the tail.
#[test]
fn glued_closer_splits_at_the_keyword() {
    assert_eq!(
        shape(&tail(b"while :; do :; done>f")),
        vec![(1, RedirectDirection::Write, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done>>f")),
        vec![(1, RedirectDirection::Append, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail_kw(b"case x in y) :;; esac>f", b"esac")),
        vec![(1, RedirectDirection::Write, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail_kw(b"if :; then :; fi>\"f\"", b"fi")),
        vec![(1, RedirectDirection::Write, b"f".to_vec())]
    );
    assert_eq!(
        shape(&tail(b"while :; do :; done>\"f\"")),
        vec![(1, RedirectDirection::Write, b"f".to_vec())]
    );
}

/// The leniency boundary: the walk **stops** at a plain word (task #405's) and
/// at a `<<`-shaped operator (task #408's), so both keep the behavior their
/// pins record, and a block with no tail has no redirections at all.
#[test]
fn the_walk_stops_at_a_word_and_at_a_heredoc_operator() {
    assert!(shape(&tail(b"while :; do :; done")).is_empty());
    assert!(shape(&tail(b"while :; do :; done x")).is_empty());
    assert!(shape(&tail(b"while :; do :; done \"c\"")).is_empty());
    assert!(shape(&tail(b"while :; do :; done <<T")).is_empty());
    assert!(shape(&tail(b"while :; do :; done <<<hi")).is_empty());
    // The io-number form of the same operator is the here-doc shape too.
    assert!(shape(&tail(b"while :; do :; done 3<<T")).is_empty());
    // A quoted operator is word data, so no redirect: the tail stops at it.
    assert!(shape(&tail(b"while :; do :; done \">\" f")).is_empty());
    // A redirect is collected up to the first stop, in order.
    assert_eq!(
        shape(&tail(b"while :; do :; done > f x")),
        vec![(1, RedirectDirection::Write, b"f".to_vec())]
    );
}

/// A redirect operator with no operand is the house parse error (bash: rc 2),
/// and the tail propagates it in the bare and fd-qualified forms.
#[test]
fn an_operator_with_no_operand_is_the_redirect_error() {
    assert_eq!(
        tail_err(b"while :; do :; done >"),
        "invalid redirect syntax"
    );
    assert_eq!(
        tail_err(b"while :; do :; done 2>"),
        "invalid redirect syntax"
    );
    assert_eq!(
        tail_err(b"while :; do :; done >|"),
        "invalid redirect syntax"
    );
}

/// [`is_closer`]: the bare keyword, and the keyword glued to an operator byte.
/// A longer word (`donec`, `donef`) and the other keywords are not it.
#[test]
fn is_closer_reads_the_bare_and_glued_keyword() {
    let flags: Vec<bool> = [
        b"done".as_slice(),
        b"done>f".as_slice(),
        b"done>>f".as_slice(),
        b"done<g".as_slice(),
        b"donec".as_slice(),
        b"donef".as_slice(),
        b"fi".as_slice(),
        b"esac".as_slice(),
    ]
    .iter()
    .map(|word| {
        let mut t = tokenize_statement(word).unwrap();
        is_closer(t.first_mut().unwrap(), b"done")
    })
    .collect();
    assert_eq!(
        flags,
        vec![true, true, true, true, false, false, false, false]
    );
}

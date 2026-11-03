//! One row per `&>`-family arm spelling at a closer (task #409). Every assertion
//! names `extra_to`, because that field is what makes a combined def: the
//! three-tuple `shape()` the sibling test files use cannot see it, so a `&>` def
//! would pass for a plain `>` def (LESSONS "The `&>` family is one operator").
//! Rows that are **not** bash parity name their owner.

#![allow(clippy::unwrap_used)]
use super::*;
use crate::parse::block_tail::{closer_index, redirects};
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDirection, RedirectSource};
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

/// The walk over `b`'s tail, exactly as a builder's call site runs it: the tokens
/// from the closing keyword `kw` on, and the block `text` the builder was handed
/// (these rows are single-physical-line blocks, so `b` is both).
fn run(b: &[u8], kw: &[u8]) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let text = ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(text.as_bytes().unwrap()).unwrap();
    let at = closer_index(&tokens, kw).unwrap();
    redirects(kw, tokens.get(at..).unwrap_or(&[]), &text)
}

/// `(fd, extra_to, direction, target)` of each def — `extra_to` is the half the
/// shape helpers elsewhere cannot see, so this file spells its own.
fn defs(b: &[u8], kw: &[u8]) -> Vec<(i32, Option<i32>, RedirectDirection, Vec<u8>)> {
    run(b, kw)
        .unwrap()
        .iter()
        .map(|d| (d.export_to, d.extra_to, d.direction, target(&d.source)))
        .collect()
}

fn target(source: &RedirectSource) -> Vec<u8> {
    match source {
        RedirectSource::Path { path, .. } => path.as_bytes().unwrap_or(&[]).to_vec(),
        RedirectSource::Var(path) => {
            let mut t = b"var:".to_vec();
            t.extend(path.as_bytes().unwrap_or(&[]));
            t
        }
        other => panic!("not a path/var source: {other:?}"),
    }
}

fn err(b: &[u8], kw: &[u8]) -> String {
    match run(b, kw) {
        Ok(defs) => panic!("expected a parse error for {b:?}, got {defs:?}"),
        Err(e) => e.current_context().to_string(),
    }
}

/// One row per arm spelling — bare and attached, write and append, spaced and
/// glued to the closer — each landing one combined def: fd 1 as `export_to`,
/// fd 2 as `extra_to`, fed by the same open.
#[test]
fn each_family_spelling_lands_one_combined_def() {
    let f = b"f".to_vec();
    for (kw, line, dir) in [
        (
            b"done".as_slice(),
            b"while :; do :; done &> f".as_slice(),
            RedirectDirection::Write,
        ),
        (
            b"done".as_slice(),
            b"while :; do :; done &>> f".as_slice(),
            RedirectDirection::Append,
        ),
        (
            b"done".as_slice(),
            b"while :; do :; done&>f".as_slice(),
            RedirectDirection::Write,
        ),
        (
            b"done".as_slice(),
            b"while :; do :; done&>>f".as_slice(),
            RedirectDirection::Append,
        ),
        (
            b"fi".as_slice(),
            b"if :; then :; fi&>f".as_slice(),
            RedirectDirection::Write,
        ),
        (
            b"esac".as_slice(),
            b"case x in x) :;; esac&>f".as_slice(),
            RedirectDirection::Write,
        ),
    ] {
        assert_eq!(
            defs(line, kw),
            vec![(1, Some(2), dir, f.clone())],
            "{line:?}"
        );
    }
}

/// The bare operator consumes its operand token and the attached one does not, so
/// the two-def tail comes out in the order it was **written** either way (POSIX
/// #2.5, `bg_redirect::insert_redirect`'s rule) — the pin for the consumption the
/// `extra` count reports.
#[test]
fn the_operand_consumption_keeps_both_orders() {
    for (kw, line, want) in [
        (
            b"done".as_slice(),
            b"while :; do :; done &> f 2> g".as_slice(),
            vec![
                (1, Some(2), RedirectDirection::Write, b"f".to_vec()),
                (2, None, RedirectDirection::Write, b"g".to_vec()),
            ],
        ),
        (
            b"done".as_slice(),
            b"while :; do :; done 2> g &> f".as_slice(),
            vec![
                (2, None, RedirectDirection::Write, b"g".to_vec()),
                (1, Some(2), RedirectDirection::Write, b"f".to_vec()),
            ],
        ),
    ] {
        assert_eq!(defs(line, kw), want, "{line:?}");
    }
}

/// The `%var` target arm stays the **fd-variable** source: bash reads the same
/// word as the file name `%v`, and that divergence is the family's, documented in
/// POSIX_COMPATIBILITY (">%var and &>%var redirect to the descriptor") and pinned
/// at the simple-command level in `tests/redirect.rs`. Not fixed here.
#[test]
fn the_var_target_stays_the_fd_variable() {
    assert_eq!(
        defs(b"while :; do :; done &>%v", b"done"),
        vec![(1, Some(2), RedirectDirection::Write, b"var:v".to_vec())]
    );
    assert_eq!(
        defs(b"while :; do :; done &>>%v", b"done"),
        vec![(1, Some(2), RedirectDirection::Append, b"var:v".to_vec())]
    );
}

/// The family's **pidvar** half (`&>&name`, `&>|&name`) is refused at a closer:
/// bash rejects the line (rc 2), the house convention reports at rc 1, and the
/// wording is the one the group's tail already gives every `&` shape. One row per
/// builder closer, plus the fdshell-only `wait` twin whose landing is a decision by
/// sharing (`tests/block_redirect.rs` pins its run).
#[test]
fn the_pidvar_arm_reports_at_every_closer() {
    let unexpected = "unexpected character in this context";
    for (kw, line) in [
        (b"done".as_slice(), b"while :; do :; done &>&p".as_slice()),
        (b"done".as_slice(), b"while :; do :; done &>|&p".as_slice()),
        (b"fi".as_slice(), b"if :; then :; fi &>&p".as_slice()),
        (b"esac".as_slice(), b"case x in x) :;; esac &>&p".as_slice()),
        (
            b"done".as_slice(),
            b"wait\n readable %rd) echo hi ;;\ndone &>&p".as_slice(),
        ),
    ] {
        assert_eq!(err(line, kw), unexpected, "{line:?}");
    }
}

/// The shapes the family parser itself refuses: a `|` absorbed behind `&>` (bash:
/// rc 2, `syntax error near unexpected token '|'`) and a bare operator with no
/// operand (bash: rc 2) both land on the house redirect error.
#[test]
fn the_clobber_and_no_operand_shapes_are_the_redirect_error() {
    let invalid = "invalid redirect syntax";
    for (kw, line) in [
        (b"done".as_slice(), b"while :; do :; done &>|f".as_slice()),
        (b"fi".as_slice(), b"if :; then :; fi &>|f".as_slice()),
        (b"done".as_slice(), b"while :; do :; done &>".as_slice()),
        (b"done".as_slice(), b"while :; do :; done &>>".as_slice()),
    ] {
        assert_eq!(err(line, kw), invalid, "{line:?}");
    }
}

/// A word behind the family's operator stays [`super::word`]'s report — the arm
/// takes the operator and its operand, then hands the word to the judge, which
/// reports it at the house rc 1 (bash: rc 2). The quoted operator word is the same
/// judge: the family's quoting gate declines it, so it is word data (task #336's
/// reading, shared with the group).
#[test]
fn a_word_behind_the_operator_stays_the_word_report() {
    for (kw, line) in [
        (b"done".as_slice(), b"while :; do :; done &> f g".as_slice()),
        (b"done".as_slice(), b"while :; do :; done '&>'%v".as_slice()),
    ] {
        assert_eq!(
            err(line, kw),
            format!(
                "'{}' must be followed by a separator (';', a newline, '&', '|') — unexpected word",
                core::str::from_utf8(kw).unwrap()
            ),
            "{line:?}"
        );
    }
}

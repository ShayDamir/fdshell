#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::token::tokenize_statement;

/// Tokenize a whole statement and run the control-keyword detector on it.
fn control(line: &[u8]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let tokens = tokenize_statement(line).unwrap();
    detect_control(&tokens)
}

fn count_of(line: &[u8]) -> u32 {
    match control(line).unwrap() {
        Some(ParsedLine::Break(n)) | Some(ParsedLine::Continue(n)) => n,
        _ => panic!("expected break/continue"),
    }
}

fn fails_with(line: &[u8], pred: impl Fn(&ParseError) -> bool) -> bool {
    match control(line) {
        Err(err) => pred(err.current_context()),
        Ok(_) => false,
    }
}

#[test]
fn bare_break_and_continue_default_to_count_one() {
    assert!(matches!(
        control(b"break").unwrap(),
        Some(ParsedLine::Break(1))
    ));
    assert!(matches!(
        control(b"continue").unwrap(),
        Some(ParsedLine::Continue(1))
    ));
}

#[test]
fn explicit_counts_are_parsed() {
    assert!(matches!(
        control(b"break 2").unwrap(),
        Some(ParsedLine::Break(2))
    ));
    assert!(matches!(
        control(b"continue 3").unwrap(),
        Some(ParsedLine::Continue(3))
    ));
}

#[test]
fn accepts_plus_prefix_and_leading_zeros() {
    assert_eq!(count_of(b"break +2"), 2);
    assert_eq!(count_of(b"break 0100"), 100);
}

#[test]
fn non_positive_or_non_numeric_counts_are_rejected() {
    for line in [
        &b"break 0"[..],
        b"break -1",
        b"break 1.5",
        b"break abc",
        b"break 0x2",
        b"break 99999999999999999999",
        b"break $n",
        b"break \"2abc\"",
    ] {
        assert!(
            fails_with(line, |e| matches!(e, ParseError::LoopCountInvalid)),
            "line: {line:?}"
        );
    }
}

#[test]
fn more_than_one_argument_is_rejected() {
    assert!(fails_with(b"break 2 3", |e| {
        matches!(e, ParseError::BreakTakesAtMostOneArgument)
    }));
    assert!(fails_with(b"continue 1 2", |e| {
        matches!(e, ParseError::ContinueTakesAtMostOneArgument)
    }));
}

#[test]
fn a_glued_count_is_a_command_word_not_the_builtin() {
    // `break2` is one word, so `detect_control` sees no `break` keyword.
    assert!(control(b"break2").unwrap().is_none());
}

#[test]
fn count_stays_a_separate_token_in_a_loop_body() {
    let inner = tokenize_statement(b"break 2").unwrap();
    assert!(matches!(
        detect_control(&inner).unwrap(),
        Some(ParsedLine::Break(2))
    ));
    assert_eq!(inner.len(), 2, "break 2 must tokenize as 2 tokens");
}

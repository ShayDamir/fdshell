#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::backtick;
use super::fields::*;
use super::word_origin;
use crate::state::ShellState;
use sys::{Origin, ShortCStr};

#[test]
fn is_backtick_requires_both_ends() {
    // Mutant MISSED 6 (old `is_cmd_subst`): replace && with ||. If && → ||,
    // "`hello" (only the first backtick) would match.
    assert!(!is_backtick(b"`hello"));
    assert!(!is_backtick(b"hello`"));
    assert!(is_backtick(b"`hello`"));
    assert!(is_backtick(b"`a`"));
    // A `$(…)` word is no longer a command-substitution word here (task #131):
    // it goes through the argument substitution pipeline.
    assert!(!is_backtick(b"$(echo hello)"));
    assert!(!is_backtick(b""));
    assert!(!is_backtick(b"`"));
}

#[test]
fn strip_delims_removes_the_backticks() {
    assert_eq!(strip_delims(b"`echo hi`"), b"echo hi");
    assert_eq!(strip_delims(b"``"), b"");
}

/// A quoted whole-word backtick whose body prints nothing is exactly one
/// empty word; unquoted, the same body yields no word at all.
#[test]
fn backtick_quoted_empty_output_is_one_empty_field() {
    let cell = sys::fork_cell::ForkCell::new(ShellState::new());
    let quoted = backtick::expand(&ShortCStr::from(c"`true`"), true, &cell).unwrap();
    assert_eq!(quoted.len(), 1);
    assert!(quoted[0].is_empty());
    let unquoted = backtick::expand(&ShortCStr::from(c"`true`"), false, &cell).unwrap();
    assert!(unquoted.is_empty());
}

#[test]
fn split_whitespace_splits_on_all_whitespace_types() {
    // Mutant MISSED 8,9: replace || with && in split_whitespace. If || → &&,
    // no single byte is both space AND tab, so whitespace would never be
    // detected → the whole input becomes one word.
    let result = split_whitespace(b"hello\tworld\nfoo\rbar").unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(result[0].as_bytes().unwrap(), b"hello");
    assert_eq!(result[1].as_bytes().unwrap(), b"world");
    assert_eq!(result[2].as_bytes().unwrap(), b"foo");
    assert_eq!(result[3].as_bytes().unwrap(), b"bar");
}

#[test]
fn word_origin_leading_tilde_is_home() {
    assert_eq!(
        word_origin(b"~/src"),
        Origin::EnvVar(ShortCStr::from(c"HOME"))
    );
    assert_eq!(word_origin(b"~"), Origin::EnvVar(ShortCStr::from(c"HOME")));
}

#[test]
fn word_origin_interior_tilde_is_shell() {
    assert_eq!(word_origin(b"a~b"), Origin::Shell);
}

#[test]
fn word_origin_command_substitution_is_command_output() {
    assert_eq!(word_origin(b"$(echo hi)"), Origin::CommandOutput);
    assert_eq!(word_origin(b"a`echo hi`b"), Origin::CommandOutput);
}

#[test]
fn word_origin_literal_word_is_shell() {
    assert_eq!(word_origin(b"plain"), Origin::Shell);
    assert_eq!(word_origin(b""), Origin::Shell);
}

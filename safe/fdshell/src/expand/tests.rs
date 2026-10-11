#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use alloc::vec;
use alloc::vec::Vec;
use sys::fork_cell::ForkCell;

use super::backtick;
use super::fields::*;
use super::word_origin;
use crate::state::ShellState;
use sys::{Origin, ShortCStr};

/// A whole-word backtick's quote mask: one bit per byte of the word, all
/// quoted or all unquoted (the arm only ever sees whole-word backticks).
fn mask(word: &ShortCStr, quoted: bool) -> Vec<bool> {
    vec![quoted; word.as_bytes().unwrap().len()]
}

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
    let cell = ForkCell::new(ShellState::new());
    let word = ShortCStr::from(c"`true`");
    let quoted = backtick::expand(&word, &mask(&word, true), true, &cell).unwrap();
    assert_eq!(quoted.len(), 1);
    assert!(quoted[0].is_empty());
    let unquoted = backtick::expand(&word, &mask(&word, false), false, &cell).unwrap();
    assert!(unquoted.is_empty());
}

/// The quoted-empty rule comes from the word's `quoted` flag, not from the
/// mask: `""` followed by a backtick has an all-unquoted mask (`""`
/// contributes no bytes) and is still exactly one empty word.
#[test]
fn backtick_quoted_flag_with_unquoted_mask_is_one_empty_field() {
    let cell = ForkCell::new(ShellState::new());
    let word = ShortCStr::from(c"`true`");
    let fields = backtick::expand(&word, &mask(&word, false), true, &cell).unwrap();
    assert_eq!(fields.len(), 1);
    assert!(fields[0].is_empty());
}

/// A fully quoted command substitution is ONE word whatever `IFS` is: its
/// space is data, not a split point (task #132, measured against bash).
#[test]
fn backtick_quoted_output_is_one_field() {
    let cell = ForkCell::new(ShellState::new());
    let word = ShortCStr::from(c"`echo a b`");
    let expected = ShortCStr::from(c"a b");
    for ifs in [
        ShortCStr::from(c" \t\n"),
        ShortCStr::from(c","),
        ShortCStr::new(),
    ] {
        cell.borrow_mut().unwrap().ifs = ifs.clone();
        let fields = backtick::expand(&word, &mask(&word, true), true, &cell).unwrap();
        assert_eq!(fields, core::slice::from_ref(&expected), "ifs={ifs}");
    }
}

/// Unquoted, the output is split by the pipeline's `IFS` splitter: every `IFS`
/// byte delimits, non-whitespace ones included, and `IFS=` disables the split
/// (task #132, measured against bash).
#[test]
fn backtick_unquoted_output_splits_on_ifs() {
    let cell = ForkCell::new(ShellState::new());
    let word = ShortCStr::from(c"`echo a b`");
    let (a, b) = (ShortCStr::from(c"a"), ShortCStr::from(c"b"));
    assert_eq!(
        backtick::expand(&word, &mask(&word, false), false, &cell).unwrap(),
        [a.clone(), b.clone()]
    );
    cell.borrow_mut().unwrap().ifs = ShortCStr::from(c",");
    // `IFS` is set to `,`, so the body's `,` is quoted and reaches the output.
    let word = ShortCStr::from(c"`printf %s \"a,b\"`");
    assert_eq!(
        backtick::expand(&word, &mask(&word, false), false, &cell).unwrap(),
        [a, b]
    );
    cell.borrow_mut().unwrap().ifs = ShortCStr::new();
    assert_eq!(
        backtick::expand(&word, &mask(&word, false), false, &cell).unwrap(),
        [ShortCStr::from(c"a,b")]
    );
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

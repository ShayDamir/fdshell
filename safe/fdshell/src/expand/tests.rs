#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::fields::*;
use super::word_origin;
use sys::Origin;
use sys::ShortCStr;

/// The classifier vocabulary of the whole-word backtick shape. The for-list
/// arm that used it is gone (task #216 expands backticks in the argument
/// pipeline), so these pins guard the classifier itself, not an arm.
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

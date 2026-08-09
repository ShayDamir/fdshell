#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

#[test]
fn is_cmd_subst_backtick_requires_both_ends() {
    // Mutant MISSED 6: replace && with || in is_cmd_subst line 30
    // If && → ||, "`hello" (only first backtick) would match
    assert!(!is_cmd_subst(b"`hello"));
    assert!(!is_cmd_subst(b"hello`"));
    assert!(is_cmd_subst(b"`hello`"));
}

#[test]
fn is_cmd_subst_dollar_paren_requires_both_ends() {
    // Mutant MISSED 7: replace && with || in is_cmd_subst line 31
    // If && → ||, "$(" without closing ) would match (due to || precedence)
    assert!(!is_cmd_subst(b"$("));
    assert!(!is_cmd_subst(b"hello)"));
    assert!(is_cmd_subst(b"$(echo hello)"));
}

#[test]
fn is_cmd_subst_minimal_backtick() {
    // Minimal valid backtick command substitution
    assert!(is_cmd_subst(b"`a`"));
    assert!(!is_cmd_subst(b"`a")); // missing closing backtick
}

#[test]
fn split_whitespace_splits_on_all_whitespace_types() {
    // Mutant MISSED 8,9: replace || with && in split_whitespace line 46
    // If || → &&, no single byte can be both space AND tab simultaneously
    // so whitespace would never be detected → entire input becomes one word
    let result = split_whitespace(b"hello\tworld\nfoo\rbar").unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(result[0].as_bytes().unwrap(), b"hello");
    assert_eq!(result[1].as_bytes().unwrap(), b"world");
    assert_eq!(result[2].as_bytes().unwrap(), b"foo");
    assert_eq!(result[3].as_bytes().unwrap(), b"bar");
}

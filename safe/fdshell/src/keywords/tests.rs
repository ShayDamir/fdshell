#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

#[test]
fn keyword_delta_closer_with_extra_boundary() {
    assert_eq!(keyword_delta(b"fi|", b"fi|", 3), Some(-1));
    assert_eq!(keyword_delta(b"done&", b"done&", 5), Some(-1));
    assert_eq!(keyword_delta(b"esac|", b"esac|", 5), Some(-1));
}

#[test]
fn keyword_delta_opener_with_semi_boundary() {
    assert_eq!(keyword_delta(b"if;", b"if;", 3), Some(1));
    assert_eq!(keyword_delta(b"for;", b"for;", 4), Some(1));
}

/// The `wait` lookahead: a word on a subsequent line, or a same-line pattern
/// keyword, opens a block; a same-line pid/`$!`/name, a `;`, a quoted word,
/// or EOF is the POSIX builtin.
#[test]
fn wait_opens_block_cases() {
    assert!(wait_opens_block(b"wait\n readable %rd) x ;;", 4));
    assert!(!wait_opens_block(b"wait 123", 4));
    assert!(!wait_opens_block(b"wait", 4));
    assert!(!wait_opens_block(b"wait \"readable\"", 4));
    // A `;` after `wait` is a statement separator, not a block opener.
    assert!(!wait_opens_block(b"wait ; readable", 4));
    assert!(!wait_opens_block(b"wait; echo hi", 4));
    assert!(wait_opens_block(b"wait # c\nreadable", 4));
    assert!(wait_opens_block(b"wait after 5)", 4));
    // A same-line non-keyword (pid, `$!`, or a name) is the builtin.
    assert!(!wait_opens_block(b"wait 123abc", 4));
    assert!(!wait_opens_block(b"wait $!", 4));
    assert!(!wait_opens_block(b"wait readableX", 4));
    // A word on a subsequent line is always a block arm.
    assert!(wait_opens_block(b"wait\ndone", 4));
    assert!(wait_opens_block(b"wait\nsleeping", 4));
}

/// `keyword_delta` for `wait` is `Some(1)` (block) or `Some(0)` (builtin).
#[test]
fn keyword_delta_wait_block_vs_builtin() {
    assert_eq!(keyword_delta(b"wait", b"wait\n readable", 4), Some(1));
    assert_eq!(keyword_delta(b"wait", b"wait 123", 4), Some(0));
    assert_eq!(keyword_delta(b"wait", b"wait", 4), Some(0));
}

/// `first_word_end` stops at the first whitespace/`;`.
#[test]
fn first_word_end_stops_at_separator() {
    assert_eq!(first_word_end(b"wait 123", 0), 4);
    assert_eq!(first_word_end(b"wait;done", 0), 4);
    assert_eq!(first_word_end(b"wait", 0), 4);
    assert_eq!(first_word_end(b"wait x", 2), 6);
}

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

/// The lookahead edge cases a single-byte mutation would flip: a bare
/// `wait\n` (EOF after the newline) is the builtin, a `;` after the newline
/// is skipped (the subsequent-line word still opens a block), and a quoted
/// word on a subsequent line is data.
#[test]
fn wait_opens_block_lookahead_edges() {
    assert!(!wait_opens_block(b"wait\n", 4));
    assert!(wait_opens_block(b"wait\n; readable", 4));
    assert!(!wait_opens_block(b"wait\n\"readable\"", 4));
    assert!(!wait_opens_block(b"wait\n'readable'", 4));
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

/// `{`/`}` are the group command's delimiters: a `{` with a boundary after it
/// (whitespace, `&`, `|`, end of input) is `+1`, `}` with a boundary (incl. the
/// glued redirect forms) is `-1`, and a glued brace word is neither.
#[test]
fn keyword_delta_group_delimiters() {
    assert_eq!(keyword_delta(b"{", b"{ echo a; }", 1), Some(1));
    assert_eq!(keyword_delta(b"{ echo a; }", b"{ echo a; }", 8), Some(1));
    assert_eq!(keyword_delta(b"{\n", b"{\n echo a", 1), Some(1));
    assert_eq!(keyword_delta(b"{&x", b"{&x", 2), Some(1));
    assert_eq!(keyword_delta(b"{|x", b"{|x", 2), Some(1));
    // A glued brace word (command name or brace expansion) is not an opener.
    assert_eq!(keyword_delta(b"{a,b}", b"{a,b}", 5), None);
    assert_eq!(keyword_delta(b"{1..5}", b"{1..5}", 6), None);
    assert_eq!(keyword_delta(b"{echo", b"{echo hi; }", 5), None);
    assert_eq!(keyword_delta(b"{}", b"{}; echo hi", 2), None);
    assert_eq!(keyword_delta(b"{;", b"{; echo a; }", 2), None);
    // A bare `{` glued to `;` is the word `{;`, so the line byte rules it out.
    assert_eq!(keyword_delta(b"{", b"{; echo a; }", 1), None);
    // Closers, including the glued fd-qualified redirect forms.
    assert_eq!(keyword_delta(b"}", b"{ echo a; }", 11), Some(-1));
    assert_eq!(keyword_delta(b"};", b"{ echo a; };", 2), Some(-1));
    assert_eq!(keyword_delta(b"}>out", b"{ echo a; }>out", 10), Some(-1));
    assert_eq!(keyword_delta(b"}2>err", b"{ echo a; }2>err", 10), Some(-1));
    assert_eq!(keyword_delta(b"}echo", b"}echo", 5), None);
    assert_eq!(keyword_delta(b"}a", b"}a", 2), None);
}

/// Nested groups net out of the depth, so an inner group never closes an
/// enclosing keyword block.
#[test]
fn keyword_delta_group_nests_to_zero() {
    let line = b"{ { echo n; }; }";
    let mut depth = 0;
    for word in [b"{", b"{", b"}", b"}"] {
        depth += keyword_delta(word, line, word.len()).unwrap_or(0);
    }
    assert_eq!(depth, 0);
}

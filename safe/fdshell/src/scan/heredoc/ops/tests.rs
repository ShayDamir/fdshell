//! Tests for the byte-level `<<` operator scan, including `#` comment
//! handling (the two counts — byte-level and token-level — must agree).

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::operator_delims;

/// A `#` comment (outside quotes, at a word start) is skipped whole: a `<<`
/// inside it is not an operator.
#[test]
fn comment_hides_operators() {
    let line = b"cat <<A # <<B\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"A");
}

/// A `#` mid-word is a literal byte, not a comment start: the operator after
/// the word is still found.
#[test]
fn comment_mid_word_is_not_comment() {
    let line = b"echo a#b <<X";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"X");
}

/// A `#` comment with no trailing newline ends the scan: nothing after it on
/// the line is an operator.
#[test]
fn comment_without_newline_ends_scan() {
    let line = b"cat <<A # <<B";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"A");
}

/// The comment skip lands just past the comment's newline: an operator on the
/// next line of the range is still found.
#[test]
fn comment_skip_resumes_on_next_line() {
    let line = b"cat <<A ##\ncat <<B";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].delim, b"A");
    assert_eq!(ops[1].delim, b"B");
}

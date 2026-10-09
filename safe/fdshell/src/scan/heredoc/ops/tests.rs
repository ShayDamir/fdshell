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

/// POSIX #4.1: the escape pair `\<` folds to a literal `<`, so `echo a\<<X`
/// keeps the word `a<<X` and has no operator (the tokenizer agrees: the word
/// is one token, no `<<` is emitted).
#[test]
fn escaped_operator_byte_is_not_an_operator() {
    let line = b"echo a\\<<X";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0, "the `\\<` pair shields the `<`");
    // A real operator after the word is still found.
    let line = b"echo a\\< <<X";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"X");
}

/// A bare delimiter word folds its unquoted escape pairs (`<<E\OF` delimits
/// `EOF`), and a trailing `\` in the delimiter keeps its backslash.
#[test]
fn delimiter_folds_escape_pairs() {
    let line = b"cat <<E\\OF\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"EOF");
    assert!(!ops[0].strip);
    let line = b"cat <<\\!EOF\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"!EOF");
}

/// A quoted delimiter keeps the pair inside the quotes (POSIX #4.2), and the
/// `<<-` marker folds the escape pairs outside the quotes.
#[test]
fn quoted_delimiter_keeps_the_pair() {
    let line = b"cat <<\"E\\OF\"\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"E\\OF");
    assert!(ops[0].quoted);
    let line = b"cat <<-E\\OF\n";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops[0].delim, b"EOF");
    assert!(ops[0].strip);
    assert!(!ops[0].quoted);
}

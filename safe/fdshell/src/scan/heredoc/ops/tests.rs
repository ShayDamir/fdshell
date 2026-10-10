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

/// POSIX #2.2 `>|`: the `|` absorbed into the clobber operator is an operator
/// byte, so it neither breaks a word nor forms a pipeline position. The
/// token-level agreement of these lines is asserted in
/// `parse/heredoc/tests.rs` (the byte scan cannot reach the token layer).
#[test]
fn clobber_pipe_is_an_operator_byte_not_a_pipeline() {
    // `cat`, `>|`, `q`, `<<EOF` — the `<<` word starts at the space after `q`.
    let line = b"cat >| q <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // The `<<` word starts at the space after the absorbed `>|` token, and the
    // absorbed `|` is not a pipeline position.
    let line = b"cat >| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // `a>|<<EOF` is one token: no word-break byte precedes the `<<`.
    let line = b"echo a>|<<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0);

    // A pipeline pipe that is not `>`-preceded keeps `<<` a command word.
    let line = b"a | <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0);

    // The `k == 0` edge: a `|` at the run start (with only whitespace between
    // it and the `<<`) is a pipeline position, so the `>`-preceded test must
    // not underflow.
    let line = b"| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0);

    // A `>|` at the run start: the absorbed `|` is not a pipeline position (the
    // `k == 0` guard is not reached, and `clobber_pipe` sees the `>` at k-1),
    // but the count is 0 for the pre-existing `seen_word` reason — `is_word_break`
    // ends the word at `>`/`|`, so no command word has been seen. The token
    // layer counts 1 here; that divergence predates `>|` and is the same class
    // as the bare `> <<EOF` line.
    let line = b">| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0);

    // The shared rule reads the word's raw bytes, so a quoted `>` does not
    // terminate an operator word: `cat "a>"| <<EOF` counts 0 at both levels
    // (`echo "a>"|b` is a pipeline on master and bash, rc 1 and 127), and with
    // a word after the pipe the `<<` is an operator: `echo "a>"|b <<EOF` counts 1.
    let line = b"cat \"a>\"| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 0, "the quoted `>` is not an operator byte");
    let line = b"echo \"a>\"|b <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // An escape pair shields its second byte, and an unquoted operator byte
    // earlier in the word disqualifies it, so neither word is `>`-terminated
    // and the `|` is a pipeline position: 0 at both levels (bash is a syntax
    // error at `<<`, rc 2; master measures rc 1 for both lines).
    let line = b"cat x\\>| <<EOF";
    assert_eq!(operator_delims(line, 0, line.len()).unwrap().len(), 0);
    let line = b"cat a>b>| <<EOF";
    assert_eq!(operator_delims(line, 0, line.len()).unwrap().len(), 0);
    // A word that carries an escape pair and ends at a real `>` is
    // `>`-terminated, so the `|` is absorbed and the `<<` counts 1 (the token
    // layer counts 1 for the same line).
    let line = b"cat x\\>a>| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // The mask is read from the line start, not from the word's start: the word
    // here begins with the closing `"`, and the quoted `>` inside `"a>"` is not
    // an operator byte, so `x\y>` terminates the word and the `|` is absorbed
    // (count 1 at both levels). A word-local mask scan would read that closing
    // quote as opening and count 0 at the byte level.
    let line = b"cat \"a>\"x\\y>| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");

    // The escape pair is skipped only outside quotes: inside `"a\>"` the `>` is
    // quoted, so the word runs to the real `>` after `x` and the `|` is
    // absorbed (count 1 at both levels). Skipping the pair inside quotes would
    // make the quoted `>` an unquoted operator byte and count 0.
    let line = b"cat \"a\\>\"x>| <<EOF";
    let ops = operator_delims(line, 0, line.len()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].delim, b"EOF");
}

#![allow(clippy::unwrap_used)]
use super::{parse_here_string, word_indices};
use crate::error::parse::ParseError;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;

fn indices(line: &[u8]) -> Vec<usize> {
    let tokens = tokenize_statement(line).unwrap();
    word_indices(line, &tokens)
}

#[test]
fn bare_form_takes_next_token() {
    assert_eq!(indices(b"cat <<< {a,b}"), vec![2]);
}

#[test]
fn attached_form_is_the_operator_token() {
    assert_eq!(indices(b"cat <<<{a,b}"), vec![1]);
}

#[test]
fn quoted_part_is_one_token() {
    // `<<<""` is one word whose quoted part contributes no bytes: the raw
    // span is longer than the unquoted word, so the operator token is the
    // index, and there is no following token to take.
    assert_eq!(indices(b"cat <<<\"\""), vec![1]);
}

#[test]
fn bare_form_at_end_of_input() {
    // A bare `<<<` with no following word protects nothing.
    assert_eq!(indices(b"cat <<<"), Vec::new());
}

/// The io-number form reads the same span rule, so the attached/bare test is
/// shifted by the prefix: `3<<<{a,b}` is attached on `{a,b}` (the unshifted
/// test would read `<<{a,b}` as the operator's own tail) and `3<<< {a,b}`
/// protects the following token.
#[test]
fn io_number_prefix_shifts_the_attached_test() {
    assert_eq!(indices(b"cat 3<<<{a,b}"), vec![1]);
    assert_eq!(indices(b"cat 3<<< {a,b}"), vec![2]);
    assert_eq!(indices(b"cat 003<<<{a,b}"), vec![1]);
    // `3<<<""` is one word: the prefix shift leaves the quoted empty word
    // unattached, and `word_quoted` claims the operator token.
    assert_eq!(indices(b"cat 3<<<\"\""), vec![1]);
    assert_eq!(indices(b"cat 3<<<"), Vec::new());
}

/// The word-data rows of `scan/heredoc::io::string_span_is_none_for_word_data`,
/// seen through the consumer: a quoted, escaped or glued prefix protects
/// nothing, exactly as `word_indices` says nothing when the parse reads word
/// data (the count-agreement rule, LESSONS).
#[test]
fn word_data_prefixes_protect_nothing() {
    for line in [
        b"cat \"3\"<<<x".as_slice(),
        b"cat \\3<<<x",
        b"cat 3\"<<<\"x",
        b"cat 3'<<<'x",
        b"cat x3<<<y",
        b"cat $FD<<<x",
    ] {
        assert_eq!(indices(line), Vec::new(), "line={:?}", line);
    }
}

/// The range failure is reported by the parse site alone (the span rule is
/// syntactic, like `span_operator`), and it names the prefix: the `N<<word`
/// twin is `operator_at_rejects_an_io_number_that_is_not_a_descriptor`. A
/// prefix that fits is an fd, and `2147483647` is the largest one.
#[test]
fn parse_here_string_rejects_an_io_number_that_is_not_a_descriptor() {
    let line = b"cat 99999999999999999999999<<<x";
    let tokens = tokenize_statement(line).unwrap();
    let err = parse_here_string(line, &tokens, 1).err().unwrap();
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidHereStringFd { .. }
    ));
    let line = b"cat 2147483647<<<x";
    let tokens = tokenize_statement(line).unwrap();
    let (def, _extra) = parse_here_string(line, &tokens, 1).unwrap().unwrap();
    assert_eq!(def.export_to, i32::MAX);
}

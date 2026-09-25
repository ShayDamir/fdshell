#![allow(clippy::unwrap_used)]
use super::word_indices;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;

fn indices(line: &[u8]) -> Vec<usize> {
    word_indices(&tokenize_statement(line).unwrap())
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

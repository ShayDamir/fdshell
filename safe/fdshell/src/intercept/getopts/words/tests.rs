#![allow(clippy::unwrap_used)]
use super::*;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

use sys::ShortCStr;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

fn imported(s: &str) -> ImportedStr {
    ImportedStr::shell(ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
}

fn positionals(words: &[&str]) -> VecDeque<ImportedStr> {
    words.iter().map(|w| imported(w)).collect()
}

fn operands_of(words: &[&[u8]]) -> Vec<ShortCStr> {
    words.iter().map(|w| s(w)).collect()
}

/// The given `arg…` operands win; the positional parameters serve only when the
/// operand list is empty (POSIX, and bash's shape measured by the epic).
#[test]
fn the_operand_list_wins_when_present() {
    let operands = operands_of(&[b"one"]);
    let positional = positionals(&["two"]);
    let words = Words::resolve(&operands, &positional);
    assert_eq!(words.word(1).unwrap(), s(b"one"));
}

#[test]
fn the_positional_parameters_serve_when_there_are_no_operands() {
    let operands = operands_of(&[]);
    let positional = positionals(&["two"]);
    let words = Words::resolve(&operands, &positional);
    assert_eq!(words.word(1).unwrap(), s(b"two"));
}

#[test]
fn the_index_is_one_based_into_the_chosen_list() {
    let operands = operands_of(&[b"one", b"two"]);
    let positional = positionals(&[]);
    let words = Words::resolve(&operands, &positional);
    assert_eq!(words.word(1).unwrap(), s(b"one"));
    assert_eq!(words.word(2).unwrap(), s(b"two"));
}

#[test]
fn index_zero_is_read_as_word_one() {
    // bash measures `OPTIND=0` mid-scan as "re-read word 1 from its start".
    let operands = operands_of(&[b"one"]);
    let positional = positionals(&[]);
    let words = Words::resolve(&operands, &positional);
    assert_eq!(words.word(words.normalize(0)).unwrap(), s(b"one"));
}

#[test]
fn past_the_end_there_is_no_word_and_the_answer_is_len_plus_one() {
    // The clamp is measured (task #590 §2): `OPTIND=9` on a two-word list ends
    // with OPTIND 3; the empty list ends at 1.
    let operands = operands_of(&[b"one", b"two"]);
    let positional = positionals(&[]);
    let words = Words::resolve(&operands, &positional);
    assert_eq!(words.word(3), None);
    assert_eq!(words.end_index(), 3);
    let empty_operands = operands_of(&[]);
    let empty_positional = positionals(&[]);
    let empty = Words::resolve(&empty_operands, &empty_positional);
    assert_eq!(empty.word(1), None);
    assert_eq!(empty.end_index(), 1);
}

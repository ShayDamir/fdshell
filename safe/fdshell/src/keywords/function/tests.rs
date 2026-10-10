#![allow(clippy::unwrap_used)]
use super::*;
use alloc::vec::Vec;

/// The definer name `function_def_name` reports, as bytes.
fn def(word: &[u8]) -> Option<Vec<u8>> {
    super::function_def_name(word).map(|w| w.to_vec())
}

#[test]
fn name_len_of_definer_words() {
    assert_eq!(name_len(b"f("), Some(1));
    assert_eq!(name_len(b"ab("), Some(2));
    assert_eq!(name_len(b"f"), Some(1));
    assert_eq!(name_len(b"-a"), Some(2));
    assert_eq!(name_len(b"_a1"), Some(3));
}

#[test]
fn name_len_rejects_non_definers() {
    // A `(` must be the last byte: an inner `(` is a `name(arg)` form.
    assert_eq!(name_len(b"f(x"), None);
    assert_eq!(name_len(b"("), None);
    // A brace in the word is not a name, and a word needs a byte.
    assert_eq!(name_len(b"f{"), None);
    assert_eq!(name_len(b"}"), None);
    assert_eq!(name_len(b""), None);
    // `)` is a word terminator, so it never belongs to a definer word
    // (a quoted name keeps one, e.g. the token `)b` of `function ")b" {`).
    assert_eq!(name_len(b"a)b"), None);
    assert_eq!(name_len(b")"), None);
    // Whitespace splits tokens, so a definer word never holds it.
    assert_eq!(name_len(b"a b"), None);
    assert_eq!(name_len(b"a\tb"), None);
}

/// The four accepted keyword shapes, plus the POSIX form.
#[test]
fn definer_words_of_the_accepted_forms() {
    assert_eq!(def(b"f() { echo hi"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f { echo hi"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f() {"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f () {"), Some(b"f".to_vec()));
    assert_eq!(def(b"function f ( ) {"), Some(b"f".to_vec()));
    // Extra whitespace between the name and the brace is skipped.
    assert_eq!(def(b"function f  {"), Some(b"f".to_vec()));
    // A reserved word as the name (the POSIX `if() {…}` leniency).
    assert_eq!(def(b"function if {"), Some(b"if".to_vec()));
    // Quoted names keep their quotes, and an option-shaped name is a name.
    assert_eq!(def(b"function \"f\" {"), Some(b"\"f\"".to_vec()));
    assert_eq!(def(b"function -a f {"), Some(b"-a".to_vec()));
}

#[test]
fn definer_words_of_the_rejected_forms() {
    // No name after `function`, and `function` glued to the opener.
    assert_eq!(def(b"function { echo hi"), None);
    assert_eq!(def(b"function(f) {"), None);
    assert_eq!(def(b"functionf f {"), None);
    assert_eq!(def(b"function"), None);
    // A name glued to `{`, a name with arguments, and a plain word.
    assert_eq!(def(b"function f{ echo hi"), None);
    assert_eq!(def(b"function f(x) {"), None);
    assert_eq!(def(b"foo { x }"), None);
    // The POSIX form needs its `()` opener.
    assert_eq!(def(b"function f}"), None);
    assert_eq!(def(b"() { x }"), None);
}

/// `)` is a word terminator, so a leading `)` never starts a definer word.
#[test]
fn close_paren_is_a_word_terminator() {
    assert_eq!(def(b") f {"), None);
    assert_eq!(def(b"function ) f {"), None);
    // The `)` after a POSIX `name(` ends the definer word.
    assert_eq!(def(b"f() { x }"), Some(b"f".to_vec()));
}

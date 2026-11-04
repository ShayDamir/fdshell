#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use crate::mask::MaskBit;
use crate::mask::MaskBit::{Quoted, Unquoted};
use crate::parse::Token;
use crate::parse::array_literal::words::open_word;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

/// A token word with a hand-written mask (one bit per word byte, the quote bytes
/// already consumed) and a zero-byte span list — the shape the tokenizer hands
/// the arm.
fn token(word: &[u8], mask: Vec<MaskBit>, spans: Vec<usize>) -> Token {
    (s(word), 0, word.len(), false, mask, spans)
}

fn mask(bits: &[bool]) -> Vec<MaskBit> {
    bits.iter().map(|&q| MaskBit::from_quoted(q)).collect()
}

/// A word mask of `n` bytes, every bit the same.
fn all_mask(n: usize, quoted: bool) -> Vec<MaskBit> {
    vec![MaskBit::from_quoted(quoted); n]
}

#[test]
fn an_unquoted_open_paren_opens_the_region() {
    let word = b"m=(1";
    let assign = open_word(&token(word, all_mask(4, false), Vec::new())).unwrap();
    assert_eq!(assign.name, s(b"m"));
    assert!(!assign.append);
    assert_eq!(assign.elements.len(), 1);
    assert_eq!(assign.elements[0].text, s(b"1"));
    assert_eq!(assign.elements[0].mask[0], Unquoted, "the byte is unquoted");
}

#[test]
fn the_name_may_end_with_the_append_sign() {
    let word = b"m+=(3";
    let assign = open_word(&token(word, all_mask(5, false), Vec::new())).unwrap();
    assert_eq!(assign.name, s(b"m"));
    assert!(assign.append);
    assert_eq!(assign.elements[0].text, s(b"3"));
}

#[test]
fn a_quoted_open_paren_opens_nothing() {
    // The mask at the `(` byte decides it, never the text: an all-quoted word is
    // an ordinary assignment word, as bash reads it too (task #422).
    let word = b"a=(x";
    assert!(open_word(&token(word, all_mask(4, true), Vec::new())).is_none());
}

#[test]
fn the_name_predicate_is_envassigns() {
    // The `%` namespace, an empty name and a plain word stay the other walks'
    // (task #422's scope decision).
    for word in [b"%arr=(x".as_slice(), b"=(x", b"echo"] {
        let m = all_mask(word.len(), false);
        assert!(open_word(&token(word, m, Vec::new())).is_none(), "{word:?}");
    }
}

#[test]
fn a_word_with_no_bytes_and_no_quote_material_is_no_element() {
    // bash: `a=()` has no elements, so the `(`-word contributes none…
    let word = b"a=(";
    let assign = open_word(&token(word, all_mask(3, false), Vec::new())).unwrap();
    assert!(assign.elements.is_empty());
    // …while a word whose quote pair contributed zero bytes is one (bash:
    // `a=("")` → `1`), and the span travels at the element's own start.
    let assign = open_word(&token(word, all_mask(3, false), vec![3])).unwrap();
    assert_eq!(assign.elements.len(), 1);
    assert_eq!(assign.elements[0].text, ShortCStr::new());
    assert_eq!(assign.elements[0].spans, vec![0]);
    assert!(
        assign.elements[0].mask.is_empty(),
        "the word carried no bytes"
    );
}

#[test]
fn the_first_element_keeps_its_own_mask() {
    // `a=(x "b c")` written as one word: the mask is parallel to the word text
    // (the quote bytes consumed), and the element's half is sliced at the `=`
    // plus the `(` — parse.rs's parallel plumbing, task #321.
    let word = b"a=(xb c";
    let whole = mask(&[false, false, false, true, true, true, true]);
    let assign = open_word(&token(word, whole.clone(), Vec::new())).unwrap();
    assert_eq!(assign.elements[0].text, s(b"xb c"));
    assert_eq!(
        assign.elements[0].mask,
        vec![Quoted, Quoted, Quoted, Quoted],
        "the element's bytes are protected from splitting"
    );
    assert_eq!(assign.elements[0].spans, Vec::new());
}

#[test]
fn a_quoted_span_of_the_value_is_shifted_to_the_element() {
    // The word `a=(x` carries a zero-byte span at the word index where a `""`
    // pair closed; sliced for the element, it names the element's own index.
    let word = b"a=(x";
    let whole = mask(&[false, false, false, true]);
    let assign = open_word(&token(word, whole, vec![3])).unwrap();
    assert_eq!(assign.elements[0].text, s(b"x"));
    assert_eq!(assign.elements[0].spans, vec![0], "shifted to the element");
}

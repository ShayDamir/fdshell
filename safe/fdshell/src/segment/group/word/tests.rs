#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

/// A group opener is `{` with a boundary after it; a brace-shaped word is not.
/// `word_end` is the byte just past the first word, so a bare `{` word is
/// judged by the line byte that follows it.
#[test]
fn opens_accepts_the_group_delimiter() {
    assert!(opens(b"{", b"{ echo a; }", 1));
    assert!(opens(b"{", b"{\n", 1));
    assert!(opens(b"{", b"{&x", 1));
    assert!(opens(b"{", b"{|x", 1));
    assert!(opens(b"{ echo a; }", b"{ echo a; }", 8));
}

#[test]
fn opens_rejects_words_that_are_not_the_group() {
    // Brace expansion, command names, and the glued `{;` form stay words.
    assert!(!opens(b"{a,b}", b"{a,b}", 1));
    assert!(!opens(b"{1..5}", b"{1..5}", 1));
    assert!(!opens(b"{echo", b"{echo", 1));
    assert!(!opens(b"{}", b"{}", 1));
    assert!(!opens(b"{};", b"{};", 1));
    assert!(!opens(b"{;", b"{;", 1));
    assert!(!opens(b"{echo hi; }", b"{echo hi; }", 10));
    assert!(!opens(b"x}", b"x}", 1));
    assert!(!opens(b"}", b"}", 1));
    // A lone `{` at the end of input has no boundary after it: it stays a word.
    assert!(!opens(b"{", b"{", 1));
    // A bare `{` followed by the glued `;` is the word `{;`, not an opener.
    assert!(!opens(b"{", b"{; echo a; }", 1));
}

/// The byte rule is shared by the word predicate, so the `{` mid-line is
/// recognized at the byte level with the same boundary set.
#[test]
fn opens_at_is_the_byte_form_of_the_opener() {
    assert!(opens_at(b"{ echo a; }", 0));
    assert!(opens_at(b"echo a && { echo b; }", 10));
    assert!(!opens_at(b"echo {", 5));
    assert!(!opens_at(b"{echo", 0));
    assert!(!opens_at(b"a}", 1));
}

/// A closer is `}` with a boundary after it, including the glued redirect form
/// (`}>out3`) and the operator forms that follow a group at statement level.
#[test]
fn closes_accepts_the_group_delimiter() {
    assert!(closes(b"}"));
    assert!(closes(b"};"));
    assert!(closes(b"}&"));
    assert!(closes(b"}|"));
    assert!(closes(b"}&&"));
    assert!(closes(b"}&x"));
    assert!(closes(b"} > out"));
    assert!(closes(b"}>out3"));
    assert!(closes(b"}2>err"));
    assert!(closes(b"}&-"));
    assert!(closes(b"}<in"));
    assert!(closes(b"}\n"));
}

#[test]
fn closes_rejects_words_that_are_not_the_group() {
    assert!(!closes(b"}echo"));
    assert!(!closes(b"}{"));
    assert!(!closes(b"}a"));
    assert!(!closes(b"{"));
    assert!(!closes(b"fi}"));
}

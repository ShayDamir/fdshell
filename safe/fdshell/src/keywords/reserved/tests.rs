//! Pins for the one reserved-word list of the boundary rule (task #396):
//! every listed word, one assertion each (killable), and the recognizers'
//! words that stay **out** — `wait`, `time`, the braces — plus a prefix-shaped
//! word and the empty word.

#![allow(clippy::unwrap_used)]

use super::reserved_word;

#[test]
fn every_listed_word_is_reserved() {
    assert!(reserved_word(b"case"), "`case` is on the list");
    assert!(reserved_word(b"esac"), "`esac` is on the list");
    assert!(reserved_word(b"if"), "`if` is on the list");
    assert!(reserved_word(b"fi"), "`fi` is on the list");
    assert!(reserved_word(b"for"), "`for` is on the list");
    assert!(reserved_word(b"while"), "`while` is on the list");
    assert!(reserved_word(b"until"), "`until` is on the list");
    assert!(reserved_word(b"done"), "`done` is on the list");
    assert!(reserved_word(b"then"), "`then` is on the list");
    assert!(reserved_word(b"else"), "`else` is on the list");
    assert!(reserved_word(b"elif"), "`elif` is on the list");
    assert!(reserved_word(b"do"), "`do` is on the list");
    assert!(reserved_word(b"in"), "`in` is on the list");
}

#[test]
fn the_off_list_words_are_not_reserved() {
    // `wait` is not a bash reserved word (its boundary row is the event-block
    // disambiguation) and `time` is a prefix, not a keyword word.
    assert!(!reserved_word(b"wait"), "`wait` stays off the list");
    assert!(!reserved_word(b"time"), "`time` stays off the list");
    // The group braces are the group scanner's words, not metacharacters.
    assert!(!reserved_word(b"{"), "the brace `{{` stays off the list");
    assert!(!reserved_word(b"}"), "the brace `}}` stays off the list");
    // A prefix-shaped word and the empty word.
    assert!(
        !reserved_word(b"ifconfig"),
        "a keyword prefix is not a word"
    );
    assert!(!reserved_word(b""), "the empty word is not a word");
}

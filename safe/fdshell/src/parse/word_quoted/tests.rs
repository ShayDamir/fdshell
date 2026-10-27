#![allow(clippy::unwrap_used)]
use super::{span_word, word_quoted};
use crate::parse::token::tokenize;
use alloc::vec::Vec;
use sys::ShortCStr;

/// `(word bytes, start, end)`: the unquoted word and its raw span.
fn case(word: &[u8], start: usize, end: usize) -> bool {
    word_quoted(&ShortCStr::from_vec(word.to_vec()).unwrap(), start, end)
}

#[test]
fn empty_quoted_region_is_quoted() {
    // `""`: no word bytes, a two-byte raw span.
    assert!(case(b"", 0, 2));
}

#[test]
fn quoted_word_is_quoted() {
    // `"a"`: one word byte, a three-byte raw span.
    assert!(case(b"a", 0, 3));
}

#[test]
fn two_empty_quoted_regions_are_quoted() {
    // `""""`: no word bytes, a four-byte raw span.
    assert!(case(b"", 0, 4));
}

#[test]
fn quotes_in_the_middle_of_a_word_are_quoted() {
    // `a"b"c`: three word bytes, a five-byte raw span.
    assert!(case(b"abc", 0, 5));
}

#[test]
fn escaped_bytes_count_as_quoted() {
    // `"a\ b"`: the escape keeps both bytes in the word (four), the span is six.
    assert!(case(b"a\\ b", 0, 6));
}

#[test]
fn plain_word_is_not_quoted() {
    // `abc`: word and span are the same length.
    assert!(!case(b"abc", 0, 3));
}

#[test]
fn span_is_relative_to_the_word_not_the_line() {
    // A word preceded by spaces: only `end - start` matters.
    assert!(case(b"a", 2, 5));
    assert!(!case(b"abc", 7, 10));
}

#[test]
fn attached_heredoc_operator_forms_are_quoted() {
    // `<<""` (heredoc) and `<<<""` (here-string): the operator bytes plus an
    // empty quoted region — the spans the reuse sites feed to this test.
    assert!(case(b"<<", 0, 4));
    assert!(case(b"<<<", 0, 5));
}

#[test]
fn attached_plain_operator_forms_are_not_quoted() {
    // `<<WORD` / `<<<word` without quotes: span equals word length.
    assert!(!case(b"<<X", 0, 3));
    assert!(!case(b"<<<w", 0, 4));
}

/// Single quotes are the same shape as double quotes for this test — the word
/// is quoted because the span is longer than the word's bytes (task #141).
#[test]
fn single_quoted_words_are_quoted() {
    // `''` (empty span) and `'a'`.
    assert!(case(b"", 0, 2));
    assert!(case(b"a", 0, 3));
    // `'a'b'c'`: three word bytes, a seven-byte span.
    assert!(case(b"abc", 0, 7));
    // `<<'EOF'` attached heredoc form: operator bytes plus a quoted span.
    assert!(case(b"<<EOF", 0, 7));
    assert!(!case(b"<<EOF", 0, 5));
}

/// `keyword_word` is the parse layer's keyword-recognition gate (task #247):
/// true only for a word whose *text* is the keyword **and** whose raw span
/// carries no quote byte. The quote spellings are bash's command-word rows —
/// `""if`/`''if` have no mask bit and `i"f"` is not `fully_quoted`, so only
/// the span rule catches them (see `keywords::quoted::quoted_word`).
#[test]
fn keyword_word_gates_every_quote_spelling() {
    for (line, end, want) in [
        (b"if x".as_slice(), 2usize, true),
        (b"'if' x".as_slice(), 4, false),
        (b"\"if\" x".as_slice(), 4, false),
        (b"i\"f\" x".as_slice(), 4, false),
        (b"\"\"if x".as_slice(), 4, false),
        (b"''if x".as_slice(), 4, false),
        (b"\"i\"f x".as_slice(), 4, false),
        (b" echo \"a\" if".as_slice(), 12, true),
    ] {
        let tok = (
            ShortCStr::from(c"if"),
            0,
            end,
            false,
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            super::keyword_word(&tok, line, b"if"),
            want,
            "{line:?} (end {end})"
        );
    }
    // The text must match the keyword too: the gate is not a quoting test alone.
    let tok = (ShortCStr::from(c"fi"), 0, 2, false, Vec::new(), Vec::new());
    assert!(!super::keyword_word(&tok, b"fi|", b"if"));
}

/// `span_word` is the parse layer's separator-recognition gate (task #336), read
/// on the tokenizer's **own** tokens: a real separator byte passes, every quoted
/// spelling of the same text is word data. `"";` is the mask-blindness pin (the
/// empty pair is its own token, the `;` a real one-byte one) and `a";"b` pins
/// that a glued quoted separator is one word whose text is `a;b`. The escape
/// pair stays in the **text** (`parse/token/escape.rs`), so the `\;` row pins
/// that no gate is needed there — and that nobody widens the text test into a
/// quote-removed comparison, which is what would split `echo a \; b`.
#[test]
fn span_word_gates_every_separator_spelling() {
    let at = |line: &[u8], i: usize| tokenize(line).unwrap().get(i).cloned().unwrap();
    // A `;` byte and a newline are the same token, and both pass: #355's
    // disguise survives the gate, because a separator token's span is the one
    // byte it stands for.
    assert!(span_word(&at(b"a ; b", 1), b";"));
    assert!(span_word(&at(b"a\nb", 1), b";"));
    assert!(span_word(&at(b"a ) b", 1), b")"));
    // The quoted spellings of the same text are word data.
    assert!(!span_word(&at(b"a \";\" b", 1), b";"));
    assert!(!span_word(&at(b"a ';' b", 1), b";"));
    assert!(!span_word(&at(b"a \")\" b", 1), b")"));
    assert!(!span_word(&at(b"a \"|\" b", 1), b"|"));
    assert!(!span_word(&at(b"a \"&\" b", 1), b"&"));
    // `"";`: the empty word token owns no mask bit and no separator text, the
    // `;` behind it is the real separator.
    let toks = tokenize(b"a \"\"; b").unwrap();
    assert!(!span_word(toks.get(1).unwrap(), b";"));
    assert!(span_word(toks.get(2).unwrap(), b";"));
    // `a";"b` is one word, text `a;b`: no separator-shaped text to gate.
    let glued = tokenize(b"a\";\"b").unwrap();
    assert_eq!(glued.len(), 1);
    assert!(!span_word(glued.first().unwrap(), b";"));
    // The escape pair stays in the text, so no arm sees a separator.
    assert_eq!(at(b"a \\; b", 1).0.as_bytes().unwrap(), b"\\;");
    assert!(!span_word(&at(b"a \\; b", 1), b";"));
    assert!(!span_word(&at(b"a \\) b", 1), b")"));
    assert!(!span_word(&at(b"a \\| b", 1), b"|"));
    // The unquoted pair of each quoted row above, so an inverted gate is dead.
    assert!(span_word(&at(b"a | b", 1), b"|"));
    assert!(span_word(&at(b"a & b", 1), b"&"));
    // The text half: the gate is not a quoting test alone.
    assert!(!span_word(&at(b"a ; b", 1), b"|"));
    assert!(!span_word(&at(b"a ) b", 1), b";"));
    assert!(!span_word(&at(b"a & b", 1), b";"));
}

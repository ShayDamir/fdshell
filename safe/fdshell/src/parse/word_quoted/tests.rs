#![allow(clippy::unwrap_used)]
use super::{is_the_word, operator_word, slot_rejects};
use crate::parse::Token;
use crate::parse::token::tokenize;
use alloc::vec::Vec;
use sys::ShortCStr;

/// `(word bytes, start, end)`: the unquoted word and its raw span.
fn case(word: &[u8], start: usize, end: usize) -> bool {
    ShortCStr::from_vec(word.to_vec())
        .unwrap()
        .is_shorter_than_span(start, end)
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
        let tok = Token {
            word: ShortCStr::from(c"if"),
            start: 0,
            end,
            fully_quoted: false,
            mask: Vec::new(),
            spans: Vec::new(),
        };
        assert_eq!(
            super::keyword_word(&tok, line, b"if"),
            want,
            "{line:?} (end {end})"
        );
    }
    // The text must match the keyword too: the gate is not a quoting test alone.
    let tok = Token {
        word: ShortCStr::from(c"fi"),
        start: 0,
        end: 2,
        fully_quoted: false,
        mask: Vec::new(),
        spans: Vec::new(),
    };
    assert!(!super::keyword_word(&tok, b"fi|", b"if"));
}

/// `operator_word` is the ONE standalone-operator-word predicate (task #524): a
/// token is the operator when its text is a row of the ONE set **and** it passes
/// [`Token::span_word`]'s gate — every quoted, escaped or glued spelling of the same
/// bytes is word data. The `line` is the one the tokens were tokenized from,
/// because the `|` row stays [`pipe_token_span`]'s byte test (task #261).
///
/// The `owned` half is the per-site data and must change the answer: the same
/// `&&` is the operator for a list that does not own it and word data for one
/// that does (a parameter that never changes the answer is an unkilleable mutant).
#[test]
fn operator_word_reads_the_gate_and_the_owners_set() {
    let at = |line: &[u8], i: usize| tokenize(line).unwrap().get(i).cloned().unwrap();
    let owns_nothing = |_w: &[u8]| false;
    for (line, i, want) in [
        // Every row of the ONE set, spelled out unquoted.
        (b"a && b".as_slice(), 1usize, Some(b"&&".as_slice())),
        (b"a & b".as_slice(), 1, Some(b"&".as_slice())),
        (b"a | b".as_slice(), 1, Some(b"|".as_slice())),
        (b"a ; b".as_slice(), 1, Some(b";".as_slice())),
        (b"a ( b".as_slice(), 1, Some(b"(".as_slice())),
        (b"a ) b".as_slice(), 1, Some(b")".as_slice())),
        // The same bytes quoted, escaped or glued: the gate says word data.
        (b"a \"\" b".as_slice(), 1, None),
        (b"a \"&&\" b".as_slice(), 1, None),
        (b"a '|' b".as_slice(), 1, None),
        (b"a \\& b".as_slice(), 1, None),
        (b"a \\| b".as_slice(), 1, None),
        (b"ax&& b".as_slice(), 0, None),
        // Not a row of the set at all.
        (b"a x b".as_slice(), 1, None),
    ] {
        assert_eq!(
            operator_word(&at(line, i), line, &owns_nothing),
            want,
            "{line:?} (token {i})"
        );
    }
    // The owners' half flips the answer for the very same tokens.
    let owns = |w: &[u8]| w == b"&&" || w == b"|";
    assert_eq!(operator_word(&at(b"a && b", 1), b"a && b", &owns), None);
    assert_eq!(operator_word(&at(b"a | b", 1), b"a | b", &owns), None);
    assert_eq!(
        operator_word(&at(b"a & b", 1), b"a & b", &owns),
        Some(b"&".as_slice()),
        "a word the position does not own is still the operator"
    );
    // The `||` row of the set, on a **synthesized** token: the tokenizer emits two
    // `|` tokens, so no real token ever arrives with this text — the row is the
    // byte-level set's spelling (task #396), and a set row nobody reaches is dead
    // data. The span equals the text, so the gate passes and the row answers.
    let tok = Token {
        word: ShortCStr::from(c"||"),
        start: 0,
        end: 2,
        fully_quoted: false,
        mask: Vec::new(),
        spans: Vec::new(),
    };
    assert_eq!(
        operator_word(&tok, b"a || b", &owns_nothing),
        Some(b"||".as_slice())
    );
    let owns_pipe2 = |w: &[u8]| w == b"||";
    assert_eq!(operator_word(&tok, b"a || b", &owns_pipe2), None);
}

/// The `&&` of [`is_the_word`]'s verdict is load-bearing: the set test and the
/// span test are asked **together**, so a plain word that equals `w` but is no
/// separator shape answers `false` — the set half alone would say true.
#[test]
fn an_equal_word_that_is_no_separator_shape_is_not_the_word() {
    let line = b"echo x";
    let tokens = tokenize(line).unwrap();
    let tok_x = tokens.get(1).unwrap();
    assert!(tok_x.span_word(b"x"), "the span half says true");
    assert!(!is_the_word(tok_x, line, b"x"), "the set half says false");
}

/// [`slot_rejects`] is the word-slot rule task **#470** added (the `case` word
/// and the `for` word list call it): [`operator_word`]'s verdict **plus** the
/// tokenizer's `(`-word reading, because the tokenizer keeps `(` a word byte.
/// Each half must carry the verdict on its own (CATCHING_MUTANTS rule 8): the
/// word-opening row `(bc` is no row of the ONE set, so only the method decides
/// it; the bare `)` opens with no `(`, so only the predicate decides it; and the
/// quoted, escaped and mid-word spellings answer `false` through **both** halves
/// (the span gate's family — LESSONS: "A separator-shaped word needs the span
/// gate"). The owners'-half rows are the `owned` parameter's own pins, the same
/// rule [`operator_word`]'s test above pins for the `&&`/`|` rows.
#[test]
fn slot_rejects_pins_both_sides_of_the_or() {
    let at = |line: &[u8], i: usize| tokenize(line).unwrap().get(i).cloned().unwrap();
    let owns_nothing = |_w: &[u8]| false;
    for (line, i, want) in [
        // Word-opening only: `(bc` is no row of the ONE set — the method decides.
        (b"a (bc d".as_slice(), 1usize, true),
        // Predicate-only: the bare `)` — the predicate decides.
        (b"a ) b".as_slice(), 1, true),
        // The exact `(` row of the set: both halves answer true.
        (b"a ( b".as_slice(), 1, true),
        // Mid-word, quoted, escaped, plain: word data through both halves.
        (b"a a(b b".as_slice(), 1, false),
        (b"a \"(b\" b".as_slice(), 1, false),
        (b"a \\(b b".as_slice(), 1, false),
        (b"a x b".as_slice(), 1, false),
    ] {
        assert_eq!(
            slot_rejects(&at(line, i), line, &owns_nothing),
            want,
            "{line:?} (token {i})"
        );
    }
    // The owners' half flips the answer for the predicate-only row: a `)` the
    // position owns is word data; the same token is the operator for a position
    // that does not own it.
    let owns_paren = |w: &[u8]| w == b")";
    assert!(!slot_rejects(&at(b"a ) b", 1), b"a ) b", &owns_paren));
    assert!(slot_rejects(&at(b"a ) b", 1), b"a ) b", &owns_nothing));
    // A word-opening word is never the exact-`(` row, so no owned set can reach
    // it: the method decides alone even when the owner says the whole text.
    let owns_the_word = |w: &[u8]| w == b"(bc";
    assert!(slot_rejects(&at(b"a (bc d", 1), b"a (bc d", &owns_the_word));
}

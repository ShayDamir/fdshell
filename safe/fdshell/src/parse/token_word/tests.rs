#![allow(clippy::unwrap_used)]

use crate::mask::MaskBit;
use crate::parse::Token;
use crate::parse::token::tokenize;
use alloc::vec;
use sys::ShortCStr;

/// [`Token::span_word`] is the parse layer's separator-recognition gate (task #336), read
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
    assert!(at(b"a ; b", 1).span_word(b";"));
    assert!(at(b"a\nb", 1).span_word(b";"));
    assert!(at(b"a ) b", 1).span_word(b")"));
    // The quoted spellings of the same text are word data.
    assert!(!at(b"a \";\" b", 1).span_word(b";"));
    assert!(!at(b"a ';' b", 1).span_word(b";"));
    assert!(!at(b"a \")\" b", 1).span_word(b")"));
    assert!(!at(b"a \"|\" b", 1).span_word(b"|"));
    assert!(!at(b"a \"&\" b", 1).span_word(b"&"));
    // `"";`: the empty word token owns no mask bit and no separator text, the
    // `;` behind it is the real separator.
    let toks = tokenize(b"a \"\"; b").unwrap();
    assert!(!toks.get(1).unwrap().span_word(b";"));
    assert!(toks.get(2).unwrap().span_word(b";"));
    // `a";"b` is one word, text `a;b`: no separator-shaped text to gate.
    let glued = tokenize(b"a\";\"b").unwrap();
    assert_eq!(glued.len(), 1);
    assert!(!glued.first().unwrap().span_word(b";"));
    // The escape pair stays in the text, so no arm sees a separator.
    assert_eq!(at(b"a \\; b", 1).word.as_bytes().unwrap(), b"\\;");
    assert!(!at(b"a \\; b", 1).span_word(b";"));
    assert!(!at(b"a \\) b", 1).span_word(b")"));
    assert!(!at(b"a \\| b", 1).span_word(b"|"));
    // The unquoted pair of each quoted row above, so an inverted gate is dead.
    assert!(at(b"a | b", 1).span_word(b"|"));
    assert!(at(b"a & b", 1).span_word(b"&"));
    // The text half: the gate is not a quoting test alone.
    assert!(!at(b"a ; b", 1).span_word(b"|"));
    assert!(!at(b"a ) b", 1).span_word(b";"));
    assert!(!at(b"a & b", 1).span_word(b";"));
}

/// CATCHING_MUTANTS rule 19: the program-level claim that "a pattern token reaches
/// no raw-span reader" does not bind a helper-level call, so the helper's own
/// answer is pinned exactly — text, start, end, `fully_quoted`, mask, spans — on a
/// token with `start > 0` and a quote span behind the `(` (the row the upstream
/// rejection forbids). Values measured on the clean tree.
#[test]
fn take_open_paren_answers_with_the_exact_shifted_tuple() {
    let token: Token = Token {
        word: ShortCStr::from(c"(a*x"),
        start: 5,
        end: 9,
        fully_quoted: false,
        mask: vec![
            MaskBit::Unquoted,
            MaskBit::QuotedStart,
            MaskBit::Unquoted,
            MaskBit::Unquoted,
        ],
        spans: vec![1],
    };
    assert_eq!(
        token.take_open_paren(),
        Some(vec![Token {
            word: ShortCStr::from(c"a*x"),
            start: 6,
            end: 9,
            fully_quoted: false,
            mask: vec![MaskBit::QuotedStart, MaskBit::Unquoted, MaskBit::Unquoted],
            spans: vec![0],
        }]),
    );
    // The token *is* the `(` (POSIX's spaced `( pattern )`): nothing is left of it,
    // so the answer is the empty list, not a shifted row.
    let token: Token = Token {
        word: ShortCStr::from(c"("),
        start: 4,
        end: 5,
        fully_quoted: false,
        mask: vec![MaskBit::Unquoted],
        spans: vec![],
    };
    assert_eq!(token.take_open_paren(), Some(vec![]));
}

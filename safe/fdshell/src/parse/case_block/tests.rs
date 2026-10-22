#![allow(clippy::unwrap_used)]

use super::tokens_to_case;
use crate::mask::MaskBit;
use crate::mask::MaskBit::Unquoted;
use crate::mask::tests::runs;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The `case` word and its mask, as the parser hands them to `case_exec`.
fn word_of(line: &[u8]) -> (ShortCStr, Vec<MaskBit>) {
    let text = stext(line);
    let tokens = tokenize_statement(line).unwrap();
    let block = tokens_to_case(&tokens, &text).unwrap();
    (block.word, block.word_mask)
}

fn sc(text: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(text.to_vec()).unwrap()
}

/// The `case` word is substituted with its own mask (task #269): a quoted
/// word's bytes are all quoted.
#[test]
fn word_mask_of_a_quoted_word() {
    assert_eq!(
        word_of(b"case \"a\\ b\" in *) echo m;; esac"),
        (sc(b"a\\ b"), runs(&[(true, 4)]))
    );
}

/// An unquoted word carries an all-unquoted mask, escape pairs included (the
/// pairs stay in the token text, so `a\ b` is a 4-byte word: `a`, `\`, ` `, `b`).
#[test]
fn word_mask_of_an_unquoted_word() {
    assert_eq!(
        word_of(b"case a\\ b in *) echo m;; esac"),
        (sc(b"a\\ b"), vec![Unquoted; 4])
    );
}

/// A multi-token join inserts one `false` bit per inserted space, and every
/// token keeps its own bits: the mask stays parallel to the joined text.
#[test]
fn word_mask_of_a_joined_word() {
    let (case_word, mask) = word_of(b"case a \"b c\" d in *) echo m;; esac");
    assert_eq!(case_word, sc(b"a b c d"));
    assert_eq!(mask, runs(&[(false, 2), (true, 3), (false, 2)]));
}

/// The word region is `;`-trimmed, and an empty region keeps an empty mask —
/// `case in …` has no word at all.
#[test]
fn word_mask_of_an_empty_word_is_empty() {
    let (case_word, mask) = word_of(b"case in *) echo m;; esac");
    assert_eq!(case_word, sc(b""));
    assert!(mask.is_empty());
    let (case_word, mask) = word_of(b"case ; ; in *) echo m;; esac");
    assert_eq!(case_word, sc(b""));
    assert!(mask.is_empty());
}

/// The `in` search is quote-gated (task #247): a quoted `in` is word data, so
/// the block reports its missing `in` (bash: a syntax error, rc 2). The word
/// rules of the clause list are untouched: a complete empty block and a
/// closer-shaped **pattern** stay as they were (task #273).
#[test]
fn in_word_is_a_keyword_only_when_unquoted() {
    let clause_count = |line: &[u8]| {
        let text = stext(line);
        let tokens = tokenize_statement(line).ok()?;
        tokens_to_case(&tokens, &text).map(|b| b.clauses.len()).ok()
    };
    let missing_in = |line: &[u8]| {
        let text = stext(line);
        let tokens = tokenize_statement(line).unwrap();
        matches!(
            tokens_to_case(&tokens, &text).map(|_| ()),
            Err(report)
                if matches!(
                    report.current_context(),
                    crate::error::parse::ParseError::CaseMissingIn
                )
        )
    };
    assert_eq!(clause_count(b"case x in esac"), Some(0));
    assert_eq!(clause_count(b"case x in done) echo D;; esac"), Some(1));
    for line in [
        b"case x 'in' a) echo A;; esac".as_slice(),
        b"case x \"in\" a) echo A;; esac",
        b"case x ''in a) echo A;; esac",
    ] {
        assert!(missing_in(line), "{line:?} must report the missing `in`");
    }
}

/// A clause closed directly by `esac` is a clause with an **empty body** (task
/// #300, bash rc 0), and the clause list keeps counting after it. The complete
/// empty block stays zero clauses (the shape task #273 pins above).
#[test]
fn a_clause_closed_by_esac_is_a_clause_with_an_empty_body() {
    let block = |line: &[u8]| {
        let text = stext(line);
        let tokens = tokenize_statement(line).unwrap();
        tokens_to_case(&tokens, &text)
            .unwrap_or_else(|e| panic!("{line:?}: {}", e.current_context()))
    };
    let b = block(b"case x in a) esac");
    assert_eq!(b.clauses.len(), 1);
    let first = b.clauses.first().unwrap();
    assert!(first.body.data.is_empty());
    assert!(first.patterns.first().unwrap().text.eq_bytes(b"a"));

    let b = block(b"case x in a) echo one;; b) esac");
    assert_eq!(b.clauses.len(), 2);
    assert!(b.clauses.first().unwrap().body.data.eq_bytes(b"echo one"));
    assert!(b.clauses.get(1).unwrap().body.data.is_empty());

    assert_eq!(block(b"case x in esac").clauses.len(), 0);
}

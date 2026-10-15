#![allow(clippy::unwrap_used)]
use super::literal_indices;
use super::tokens_to_case;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn indices(line: &[u8]) -> Vec<usize> {
    literal_indices(&tokenize_statement(line).unwrap())
}

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The `case` word and its mask, as the parser hands them to `case_exec`.
fn word_of(line: &[u8]) -> (ShortCStr, Vec<bool>) {
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
        (sc(b"a\\ b"), vec![true; 4])
    );
}

/// An unquoted word carries an all-unquoted mask, escape pairs included (the
/// pairs stay in the token text, so `a\ b` is a 4-byte word: `a`, `\`, ` `, `b`).
#[test]
fn word_mask_of_an_unquoted_word() {
    assert_eq!(
        word_of(b"case a\\ b in *) echo m;; esac"),
        (sc(b"a\\ b"), vec![false; 4])
    );
}

/// A multi-token join inserts one `false` bit per inserted space, and every
/// token keeps its own bits: the mask stays parallel to the joined text.
#[test]
fn word_mask_of_a_joined_word() {
    let (case_word, mask) = word_of(b"case a \"b c\" d in *) echo m;; esac");
    assert_eq!(case_word, sc(b"a b c d"));
    assert_eq!(mask, vec![false, false, true, true, true, false, false]);
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

#[test]
fn non_case_lines_are_empty() {
    assert_eq!(indices(b"echo {a,b}"), Vec::new());
    assert_eq!(indices(b"case a b"), Vec::new());
    // Starts with `case` but does not end with `esac`: not a case line.
    assert_eq!(indices(b"case {a,b} w in"), Vec::new());
}

#[test]
fn single_clause_word_and_pattern() {
    // Tokens: case {a,b} in {x,y} ) echo hi ; ; esac
    assert_eq!(indices(b"case {a,b} in {x,y}) echo hi ;; esac"), vec![1, 3]);
}

#[test]
fn semi_between_word_and_in() {
    // The `;` belongs to the case word, not the pattern region.
    assert_eq!(
        indices(b"case {a,b}; in {x,y}) echo hi ;; esac"),
        vec![1, 2, 4]
    );
}

#[test]
fn lone_semi_before_first_pattern() {
    assert_eq!(
        indices(b"case {a,b} in ; {x,y}) echo hi ;; esac"),
        vec![1, 4]
    );
}

#[test]
fn two_clauses() {
    // Tokens: case {a,b} in {x,y} ) echo hi ; ; {z,w} ) echo ok ; ; esac
    assert_eq!(
        indices(b"case {a,b} in {x,y}) echo hi ;; {z,w}) echo ok ;; esac"),
        vec![1, 3, 9]
    );
}

#[test]
fn clause_body_with_lone_semi() {
    // A lone `;` in the clause body: the body is skipped by the `;;` pair
    // after the close paren, so only the pattern words are protected.
    // Tokens: case {a,b} in {x,y} ) a ; b c d ; ; {z,w} ) echo ok ; ; esac
    assert_eq!(
        indices(b"case {a,b} in {x,y}) a ; b c d ;; {z,w}) echo ok ;; esac"),
        vec![1, 3, 12]
    );
}

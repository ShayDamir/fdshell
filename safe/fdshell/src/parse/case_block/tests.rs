#![allow(clippy::unwrap_used)]
use super::literal_indices;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;

fn indices(line: &[u8]) -> Vec<usize> {
    literal_indices(&tokenize_statement(line).unwrap())
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

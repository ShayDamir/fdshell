#![allow(clippy::unwrap_used)]

use super::literal_indices;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;

fn indices(line: &[u8]) -> Vec<usize> {
    literal_indices(&tokenize_statement(line).unwrap(), line)
}

#[test]
fn non_case_lines_are_empty() {
    assert_eq!(indices(b"echo {a,b}"), Vec::new());
    assert_eq!(indices(b"case a b"), Vec::new());
    // Starts with `case` but does not end with `esac`: not a case line.
    assert_eq!(indices(b"case {a,b} w in"), Vec::new());
}

/// The truncation's closer may be a closer-shaped **word-list** word (`done`
/// in command position after the `;`): the walk stops there, and the guard's
/// `esac` requirement is what rejects the line. The `||` of that guard is
/// load-bearing — with `&&` the guard short-circuits on the `case` word and
/// the walk's indices are returned for a line that is not a case block at all.
#[test]
fn case_line_closed_by_a_non_esac_closer_is_empty() {
    assert_eq!(indices(b"case {a,b} in {x,y}) echo a; done"), Vec::new());
}

#[test]
fn case_line_without_in_or_close_paren_is_empty() {
    // No `in`: the word/pattern split has no start, so nothing is protected.
    assert_eq!(indices(b"case x) echo a;; esac"), Vec::new());
    // A clause region with no `)`: the walk stops at it, keeping the word.
    assert_eq!(indices(b"case x in echo ;; esac"), vec![1]);
}

#[test]
fn single_clause_word_and_pattern() {
    // Tokens: case {a,b} in {x,y} ) echo hi ; ; esac
    assert_eq!(indices(b"case {a,b} in {x,y}) echo hi ;; esac"), vec![1, 3]);
}

/// The truncation at the closing keyword is load-bearing here: in a
/// single-line `case` the clause heredoc's bodies sit **after** `esac`, so
/// without it the trailing body words break the `esac` guard and the pattern
/// is brace-expanded on the way to the parser (`case '{a,b}' in {a,b})`
/// matches the literal `{a,b}` in bash — `tests/heredoc.rs::
/// single_line_case_clause_body_heredoc_of_a_brace_pattern_is_literal`).
#[test]
fn single_clause_pattern_of_a_body_heredoc_is_protected() {
    assert_eq!(
        indices(b"case {a,b} in {x,y}) cat <<T;; esac\nbodyT\nT"),
        vec![1, 3]
    );
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

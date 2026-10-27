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

/// A `case` line whose word list closes on a non-`esac` closer (`done` in
/// command position after the `;`) is rejected. Measured (task #327) it is
/// rejected by the **truncation gate**: `list_closing_keyword_index` answers
/// `None` for this line, so the guard's `esac` test is never reached and the
/// guard's `||`→`&&` mutant is an accepted equivalent (LESSONS:
/// "`literal_indices`' clause step and pair-search window are equivalent").
/// Task #273 read this row as the pin of that `||`; it stays as the pin of the
/// truncation gate's rejection.
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

/// The `esac`-closed clause of task #300 (`case x in a) esac`, bash rc 0): this
/// walk is lenient by design (a `)` searched in `pos..esac_idx`, a missing `;;`
/// jumps to `esac_idx`), so it already covers the pattern word of a clause that
/// has no body at all. Pinned so the lenient walk and the strict clause walk of
/// `case_clause.rs` cannot drift apart on the shape.
#[test]
fn pattern_of_a_clause_closed_by_esac_is_protected() {
    // Tokens: case {a,b} in {x,y} ) esac
    assert_eq!(indices(b"case {a,b} in {x,y}) esac"), vec![1, 3]);
    // The last clause of a two-clause block, closed at `esac`.
    assert_eq!(
        indices(b"case {a,b} in {x,y}) echo hi ;; {z,w}) esac"),
        vec![1, 3, 9]
    );
}
/// The region walk reads its needles through [`span_word`](crate::parse::word_quoted::span_word)
/// (task #336), so it records the regions `case_clause::parse_clauses` actually
/// parses (the mirror property of tasks #273/#455): a quoted `")"` never closes a
/// pattern list — the region it sits in is **no** region at all, the same verdict
/// the parser reports as `case: missing ')'` — and a quoted `|` / `;` stay inside
/// the region they sit in.
#[test]
fn a_quoted_separator_keeps_the_regions_where_the_parser_reads_them() {
    // The case word and the pattern region of a normal line.
    assert_eq!(indices(b"case x in a) echo A;; esac"), vec![1, 3]);
    assert_eq!(indices(b"case x in a) esac"), vec![1, 3]);
    // A quoted `|` is pattern text: the region covers the whole pattern, token
    // for token as the unquoted two-pattern spelling does.
    assert_eq!(
        indices(b"case x in a \"|\" b) echo A;; esac"),
        indices(b"case x in a|b) echo A;; esac")
    );
    // A quoted `)` is pattern text, so the list never closes and no pattern
    // region is recorded; the ungated walk recorded one for that `)`.
    assert_eq!(indices(b"case x in a\")\" esac"), vec![1]);
    // A quoted `;` is arm text: it closes no clause, so the second `)` of the
    // line is arm text too and only the first clause has a pattern region.
    assert_eq!(
        indices(b"case x in a) echo \";\" b) echo C;; esac"),
        vec![1, 3]
    );
    // A quoted `;` as the case **word** is a word, so the walk starts normally.
    assert_eq!(indices(b"case \";\" in a) echo A;; esac"), vec![1, 3]);
}

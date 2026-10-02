//! Tests for the physical-line heredoc helpers.

use super::{line_bodies_for_line, line_end_after, operator_count};
use alloc::vec;

/// `line_end_after` finds the first unquoted newline at or after `from`.
#[test]
fn line_end_after_finds_first_newline() {
    let line = b"cat <<EOF; echo done\nbody\nEOF\n";
    assert_eq!(line_end_after(line, 0), 21);
}

/// `line_end_after` returns `line.len()` when there is no newline.
#[test]
fn line_end_after_no_newline_is_len() {
    let line = b"cat <<EOF; echo done";
    assert_eq!(line_end_after(line, 0), line.len());
}

/// `line_bodies_for_line` finds the body regions of a `;`-terminated line.
#[test]
fn line_bodies_for_line_semicolon_terminated() {
    let line = b"cat <<EOF; echo done\nbody\nEOF\n";
    let (bodies, resume) = line_bodies_for_line(line, 0, 9);
    assert_eq!(bodies, vec![(21, 30)]);
    assert_eq!(resume, 29);
}

/// `line_bodies_for_line` returns empty when the line has no operators.
#[test]
fn line_bodies_for_line_no_operators_is_empty() {
    let line = b"echo hello\n";
    let (bodies, resume) = line_bodies_for_line(line, 0, 10);
    assert!(bodies.is_empty());
    assert_eq!(resume, 10);
}

/// `operator_count` counts the `<<` operators in a run.
#[test]
fn operator_count_counts_operators() {
    let line = b"cat <<A && cat <<B";
    assert_eq!(operator_count(line, 0, 8), 1);
    assert_eq!(operator_count(line, 10, 18), 1);
    assert_eq!(operator_count(line, 0, 18), 2);
}

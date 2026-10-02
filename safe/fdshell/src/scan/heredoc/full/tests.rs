//! Tests for the full heredoc body regions.

#![allow(clippy::unwrap_used)]

use super::super::op::Operator;
use super::body_regions_full;
use alloc::vec;

/// The region spans the body lines plus the delimiter line (including its
/// terminating newline), and the resume index lands on the delimiter line's
/// newline.
#[test]
fn body_regions_full_spans_body_and_delimiter() {
    let line = b"body\nEOF\nnext\n";
    let ops = vec![Operator::new(b"EOF", false)];
    let (regions, resume) = body_regions_full(line, 0, &ops).unwrap();
    assert_eq!(regions, vec![(0, 9)]);
    assert_eq!(resume, 8);
}

/// Multiple operators yield sequential regions in operator order.
#[test]
fn body_regions_full_multiple_operators_are_sequential() {
    let line = b"a\nA\nb\nB\n";
    let ops = vec![Operator::new(b"A", false), Operator::new(b"B", false)];
    let (regions, resume) = body_regions_full(line, 0, &ops).unwrap();
    assert_eq!(regions, vec![(0, 4), (4, 8)]);
    assert_eq!(resume, 7);
}

/// A missing delimiter line is reported as the (n+1)th index.
#[test]
fn body_regions_full_missing_delimiter_is_err() {
    let line = b"body\nnothing\n";
    let ops = vec![Operator::new(b"EOF", false)];
    assert_eq!(body_regions_full(line, 0, &ops), Err(0));
}

//! Tests for the cond-list body assignment.

use super::take_bodies;
use alloc::vec;

/// `take_bodies` assigns the next `n` body bytes to a part with `n` operators.
#[test]
fn take_bodies_assigns_by_operator_count() {
    let line = b"cat <<A && cat <<B";
    let bodies = vec![vec![b'a'], vec![b'b']];
    let mut idx = 0;
    // First part: `cat <<A ` (0..8, the `&&` is at 8..10).
    let first = take_bodies(line, 0, 8, &bodies, &mut idx);
    assert_eq!(first, vec![vec![b'a']]);
    assert_eq!(idx, 1);
    // Second part: ` cat <<B` (10..18, end of line).
    let second = take_bodies(line, 10, 18, &bodies, &mut idx);
    assert_eq!(second, vec![vec![b'b']]);
    assert_eq!(idx, 2);
}

/// A part with no operators gets no body bytes.
#[test]
fn take_bodies_no_operators_gets_none() {
    let line = b"echo hello";
    let bodies = vec![vec![b'a']];
    let mut idx = 0;
    let out = take_bodies(line, 0, 10, &bodies, &mut idx);
    assert!(out.is_empty());
    assert_eq!(idx, 0);
}

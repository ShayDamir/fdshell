//! Tests for the cond-list body assignment and the group-depth boundary scan.

use super::boundary::next_boundary;
use super::take_bodies;
use crate::scan::ScanState;
use alloc::vec;

/// `next_boundary` from the start of `line`.
fn boundary_of(line: &[u8]) -> (usize, bool) {
    next_boundary(line, 0, &mut ScanState::new())
}

/// A `{ …; }` group is opaque: an `&&` inside the body belongs to the body, and
/// the split is the `&&` **after** the closing brace.
#[test]
fn operator_inside_a_group_body_is_not_a_boundary() {
    // `{ echo a && echo b; } && echo c` — the first `&&` (9) is inside the body,
    // the split is the operator at 22.
    assert_eq!(boundary_of(b"{ echo a && echo b; } && echo c"), (22, false));
    // A body `&&` with no operator after the group: no split.
    assert_eq!(boundary_of(b"{ echo a && echo b; }"), (21, false));
    // After `;` the run restarts, so the group after it is counted.
    assert_eq!(boundary_of(b"echo a; { echo b; } && echo c"), (20, false));
    // Two groups as cond-list parts: the split is the `&&` between them.
    assert_eq!(boundary_of(b"{ echo a; } && { echo b; }"), (12, false));
    // A group inside `$( … )` is the substitution's own text: the top-level
    // `&&` after the closing `)` (22) splits.
    assert_eq!(boundary_of(b"echo $( { echo a; } ) && echo c"), (22, false));
}

/// A `}` that closes nothing must clamp the depth at 0: a negative depth would
/// disable the split test for every later `&&` / `||` in the line, swallowing
/// the operator into the command's arguments.
#[test]
fn stray_closer_clamps_the_depth_at_zero() {
    // `echo x} && echo y` — the `}` at 6 closes nothing; the split is the `&&` at 8.
    assert_eq!(boundary_of(b"echo x} && echo y"), (8, false));
    assert_eq!(boundary_of(b"echo -n x} && echo y"), (11, false));
    // A brace-expansion word ends with `}`: the split stays at the `&&`.
    assert_eq!(boundary_of(b"echo {a,b} && echo ok"), (11, false));
    assert_eq!(boundary_of(b"echo {1..3} && echo ok"), (12, false));
    // Several stray closers keep the depth at 0.
    assert_eq!(boundary_of(b"echo }}} && echo y"), (9, false));
}

/// A `{` in an argument position is word data, not a group opener: the depth
/// stays 0 and the `&&` / `||` splits as it does in bash. The same clause-position
/// rule the segment scanner uses (`group_at_part_start`) governs the counter.
#[test]
fn brace_in_argument_position_is_not_a_group() {
    // `echo { && echo b` — the brace at 5 is an argument; split at the `&&` (7).
    assert_eq!(boundary_of(b"echo { && echo b"), (7, false));
    assert_eq!(boundary_of(b"echo x{ && echo y"), (8, false));
    assert_eq!(boundary_of(b"echo {{ && echo b"), (8, false));
    assert_eq!(boundary_of(b"echo { || echo b"), (7, true));
    // After `;` the run restarts, and the brace is still an argument.
    assert_eq!(boundary_of(b"x=1; echo { && echo $x"), (12, false));
    // A brace glued to a word (brace expansion) is not an opener.
    assert_eq!(boundary_of(b"echo a{b && echo c"), (9, false));
}

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

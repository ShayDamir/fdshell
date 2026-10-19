//! Tests for the physical-line heredoc helpers.

use super::{line_bodies_for_line, line_end_after, line_start};
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

/// `line_end_after` skips a newline inside quotes: the physical line
/// continues past it (there is no unquoted newline).
#[test]
fn line_end_after_quoted_newline_is_not_line_end() {
    let line = b"echo \"a\nb\"";
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

/// `line_start` is the index **past** the preceding newline: the whole-line
/// operator windows are anchored there, so the newline byte itself is never
/// inside the window (the byte scan reads it as a token byte, task #261's
/// newline rule). The no-newline and at-newline rows pin the `unwrap_or(0)`
/// and the `n + 1` arm.
#[test]
fn line_start_anchors_past_the_preceding_newline() {
    assert_eq!(line_start(b"cat <<A\n cat <<B", 9), 8);
    assert_eq!(line_start(b"a\nb\ncat", 6), 4);
    assert_eq!(line_start(b"cat <<A", 3), 0);
    assert_eq!(line_start(b"\n<<EOF", 0), 0);
    assert_eq!(line_start(b"\n<<EOF", 1), 1);
}

/// A zero-length window has no operators, so the walk's early return answers
/// `(empty, line_end)` before the `line_end - 1` read is reached. Pinned: the
/// `line_end > 0` guard on that read is **defensive and unreachable** at
/// `line_end == 0` (an empty window yields no ops, so `ops.is_empty()` returns
/// first) — its mutant is accepted, the guard stays so a debug build cannot
/// underflow if a caller ever widens the window contract.
#[test]
fn line_bodies_of_an_empty_window_are_empty() {
    assert_eq!(line_bodies_for_line(b"cat <<T\nbody\nT", 0, 0), (vec![], 0));
    assert_eq!(line_bodies_for_line(b"echo hi", 0, 0), (vec![], 0));
    assert_eq!(line_bodies_for_line(b"cat", 0, 0), (vec![], 0));
}

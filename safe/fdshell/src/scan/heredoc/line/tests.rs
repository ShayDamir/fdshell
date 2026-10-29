//! Tests for the physical-line heredoc helpers.

use super::{line_bodies_for_line, line_body_contents, line_end_after, line_start};
use alloc::vec;
use alloc::vec::Vec;

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

/// A **partial** stream (the first operator's delimiter line present, a later
/// one's missing): the line keeps the region of the operator whose body
/// arrived, the missing one claims the rest of the stream, and the resume is
/// the text end, so the claimed bytes are never scanned as commands (task #275).
#[test]
fn line_bodies_of_a_partial_stream_keep_the_complete_regions() {
    let line = b"cat <<T; cat <<E\nbodyT\nT";
    let (bodies, resume) = line_bodies_for_line(line, 0, 16);
    assert_eq!(bodies, vec![(17, 24), (25, 24)]);
    assert_eq!(resume, line.len());
    let contents = line_body_contents(line, 0, 16);
    assert_eq!(contents.1, line.len());
    let texts: Vec<&[u8]> = contents.0.iter().map(|b| b.as_slice()).collect();
    assert_eq!(texts, vec![b"bodyT\nT\n".as_slice(), b"".as_slice()]);
}

/// The line's **first** delimiter line missing: the end-of-file shape — the
/// first missing operator claims the rest of the stream by the same rule that
/// serves the operators behind it (task #275's `found == 0` exception is gone),
/// the resume is the text end, and the parse layer accepts the claim with
/// bash's end-of-file warning (bash warns twice and runs at rc 0 on this line,
/// measured 2026-10-29).
#[test]
fn line_bodies_of_a_first_missing_delimiter_claims_the_stream() {
    let line = b"cat <<E; cat <<T\nbodyT\nT";
    assert_eq!(
        line_bodies_for_line(line, 0, 16),
        (vec![(17, 24), (24, 24)], 24)
    );
    assert_eq!(
        line_body_contents(line, 0, 16),
        (vec![b"bodyT\nT\n".to_vec(), Vec::new()], 24usize)
    );
}

/// A window claims the line's allocation **after** the operators written before
/// it, and a tail claim is an entry of its own: with a tail region present the
/// `skip`/`take` stays exact, the window taking the tail claim's own text —
/// which is empty for an end-of-file region (task #364).
#[test]
fn claimed_line_bodies_skip_the_operators_written_before_the_window() {
    let line = b"cat <<T; cat <<E\nbodyT\nT";
    let (regions, bodies) = super::claimed_line_bodies(line, 0, 10, 16, 1);
    assert_eq!(regions, vec![(25, 24)]);
    assert_eq!(bodies, vec![Vec::new()]);
    // The whole window (both operators) keeps both entries, in order.
    let (all, texts) = super::claimed_line_bodies(line, 0, 0, 16, 2);
    assert_eq!(all, vec![(17, 24), (25, 24)]);
    assert_eq!(texts, vec![b"bodyT\nT\n".to_vec(), Vec::new()]);
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

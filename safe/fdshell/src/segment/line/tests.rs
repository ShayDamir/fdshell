//! Tests for the physical-line run flush.

#![allow(clippy::indexing_slicing)]

use super::flush_line;
use crate::segment::Segment;
use crate::segment::book::Run;
use alloc::vec;
use alloc::vec::Vec;

/// A buffered run with its background flag off (the tests probe the flush's
/// body/offset bookkeeping; the flag itself is pinned in `segment/bg/tests.rs`).
fn run(bytes: &[u8], off: usize) -> Run<'_> {
    Run {
        bytes,
        off,
        background: false,
    }
}

/// Check a flushed statement segment's fields.
fn check_statement(segment: &Segment<'_>, cmd: &[u8], off: usize, bodies: &[(usize, usize)]) {
    match segment {
        Segment::Statement {
            cmd: c,
            off: o,
            bodies: b,
            ..
        } => {
            assert_eq!(*c, cmd);
            assert_eq!(*o, off);
            assert_eq!(b, bodies);
        }
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("expected Statement"),
    }
}

/// A single-line flush attaches the line's body regions to the runs in
/// operator order. `line_end` is the newline's position (the segment-flush
/// convention).
#[test]
fn flush_line_attaches_bodies_in_operator_order() {
    //  c a t   < < A  & &  c a t   < < B \n b o d y A \n A \n b o d y B \n B
    //  0       7  8 9 10      17  18                 24 25 26            32 33
    let line = b"cat <<A && cat <<B\nbodyA\nA\nbodyB\nB";
    let mut segments = Vec::new();
    let mut runs = vec![run(b"cat <<A", 0), run(b"cat <<B", 11)];
    let resume = flush_line(&mut segments, line, 0, 18, &mut runs);
    assert_eq!(segments.len(), 2);
    match &segments[0] {
        Segment::Statement {
            cmd, off, bodies, ..
        } => {
            assert_eq!(*cmd, b"cat <<A");
            assert_eq!(*off, 0);
            assert_eq!(bodies, &vec![(19, 27)]);
        }
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("expected Statement"),
    }
    match &segments[1] {
        Segment::Statement {
            cmd, off, bodies, ..
        } => {
            assert_eq!(*cmd, b"cat <<B");
            assert_eq!(*off, 11);
            assert_eq!(bodies, &vec![(27, 34)]);
        }
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("expected Statement"),
    }
    assert_eq!(resume, 34);
}

/// A `;`-terminated line: the body starts after the line's own newline, not
/// at the `;`.
#[test]
fn flush_line_semicolon_terminated_line() {
    //  c a t   < < E O F ;  e c h o   d o n e \n b o d y \n E O F
    //  0       8  9        19     20           24      25     28
    let line = b"cat <<EOF; echo done\nbody\nEOF";
    let mut segments = Vec::new();
    let mut runs = vec![run(b"cat <<EOF", 0)];
    let resume = flush_line(&mut segments, line, 0, 20, &mut runs);
    check_statement(&segments[0], b"cat <<EOF", 0, &[(21, 29)]);
    assert_eq!(resume, 29);
}

/// Runs buffered from an earlier physical line (a trailing `#` comment
/// deferred the flush) are emitted body-less: matching bodies across lines
/// stays unsupported.
#[test]
fn flush_line_multi_line_runs_are_body_less() {
    let line = b"cat <<EOF # c\necho x\nbody\nEOF";
    let mut segments = Vec::new();
    // The run was buffered on physical line 0 but flushed at line 1's end.
    let mut runs = vec![run(b"cat <<EOF", 0)];
    let resume = flush_line(&mut segments, line, 14, 20, &mut runs);
    check_statement(&segments[0], b"cat <<EOF", 0, &[]);
    assert_eq!(resume, 20);
}

/// A run with no operators gets no body regions, even when the line has
/// bodies for other runs.
#[test]
fn flush_line_no_operator_run_gets_no_bodies() {
    //  e c h o   x ;   c a t   < < E O F \n b o d y \n E O F
    //  0       5  6    7      15    16  17         21      22     25
    let line = b"echo x; cat <<EOF\nbody\nEOF";
    let mut segments = Vec::new();
    let mut runs = vec![run(b"echo x", 0), run(b"cat <<EOF", 8)];
    let resume = flush_line(&mut segments, line, 0, 17, &mut runs);
    assert_eq!(segments.len(), 2);
    match &segments[0] {
        Segment::Statement {
            cmd, off, bodies, ..
        } => {
            assert_eq!(*cmd, b"echo x");
            assert_eq!(*off, 0);
            assert!(bodies.is_empty());
        }
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("expected Statement"),
    }
    match &segments[1] {
        Segment::Statement {
            cmd, off, bodies, ..
        } => {
            assert_eq!(*cmd, b"cat <<EOF");
            assert_eq!(*off, 8);
            assert_eq!(bodies, &vec![(18, 26)]);
        }
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("expected Statement"),
    }
    assert_eq!(resume, 26);
}

/// The plain-`&` split (task #313) flushes the same physical line **twice**:
/// the `&` flushes the runs it terminates with a window that ends at the `&`,
/// and the line's newline flushes the runs written behind it. Both index the
/// line's one region stream by the operators' position in the **line**, so the
/// suffix run takes region 1 and never re-claims region 0.
#[test]
fn flush_line_indexes_the_line_stream_globally() {
    //  c a t   < < A   &   c a t   < < B \n x \n A \n y \n B
    //  0       4     8   10      14  17  18  19 20 21 22  24
    let line = b"cat <<A & cat <<B\nx\nA\ny\nB";
    // The `&` flush: the prefix run, window `[0, amp)`.
    let mut segments = Vec::new();
    let mut runs = vec![run(b"cat <<A", 0)];
    let resume = flush_line(&mut segments, line, 0, 8, &mut runs);
    check_statement(&segments[0], b"cat <<A", 0, &[(18, 22)]);
    assert_eq!(resume, 21, "the newline ending the last delimiter line");

    // The line's own flush: only the suffix run is buffered, and its global
    // operator index (`cat <<B` follows one operator written before the `&`)
    // picks region 1 of the same stream.
    let mut segments = Vec::new();
    let mut runs = vec![run(b"cat <<B", 10)];
    let resume = flush_line(&mut segments, line, 0, 17, &mut runs);
    check_statement(&segments[0], b"cat <<B", 10, &[(22, 25)]);
    assert_eq!(resume, 25, "past the line's last region");
}

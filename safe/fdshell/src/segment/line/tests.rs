//! Tests for the physical-line run flush.

#![allow(clippy::indexing_slicing)]

use super::flush_line;
use crate::segment::Segment;
use alloc::vec;
use alloc::vec::Vec;

/// Check a flushed statement segment's fields.
fn check_statement(segment: &Segment<'_>, cmd: &[u8], off: usize, bodies: &[(usize, usize)]) {
    match segment {
        Segment::Statement {
            cmd: c,
            off: o,
            bodies: b,
        } => {
            assert_eq!(*c, cmd);
            assert_eq!(*o, off);
            assert_eq!(b, bodies);
        }
        Segment::Block { .. } => panic!("expected Statement"),
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
    let mut runs = vec![
        (b"cat <<A" as &[u8], 0usize),
        (b"cat <<B" as &[u8], 11usize),
    ];
    let resume = flush_line(&mut segments, line, 0, 18, &mut runs);
    assert_eq!(segments.len(), 2);
    match &segments[0] {
        Segment::Statement { cmd, off, bodies } => {
            assert_eq!(*cmd, b"cat <<A");
            assert_eq!(*off, 0);
            assert_eq!(bodies, &vec![(19, 27)]);
        }
        Segment::Block { .. } => panic!("expected Statement"),
    }
    match &segments[1] {
        Segment::Statement { cmd, off, bodies } => {
            assert_eq!(*cmd, b"cat <<B");
            assert_eq!(*off, 11);
            assert_eq!(bodies, &vec![(27, 34)]);
        }
        Segment::Block { .. } => panic!("expected Statement"),
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
    let mut runs = vec![(b"cat <<EOF" as &[u8], 0usize)];
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
    let mut runs = vec![(b"cat <<EOF" as &[u8], 0usize)];
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
    let mut runs = vec![
        (b"echo x" as &[u8], 0usize),
        (b"cat <<EOF" as &[u8], 8usize),
    ];
    let resume = flush_line(&mut segments, line, 0, 17, &mut runs);
    assert_eq!(segments.len(), 2);
    match &segments[0] {
        Segment::Statement { cmd, off, bodies } => {
            assert_eq!(*cmd, b"echo x");
            assert_eq!(*off, 0);
            assert!(bodies.is_empty());
        }
        Segment::Block { .. } => panic!("expected Statement"),
    }
    match &segments[1] {
        Segment::Statement { cmd, off, bodies } => {
            assert_eq!(*cmd, b"cat <<EOF");
            assert_eq!(*off, 8);
            assert_eq!(bodies, &vec![(18, 26)]);
        }
        Segment::Block { .. } => panic!("expected Statement"),
    }
    assert_eq!(resume, 26);
}

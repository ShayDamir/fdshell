//! Tests for the physical-line run flush.

#![allow(clippy::indexing_slicing)]

use super::flush_line;
use crate::segment::Segment;
use crate::segment::book::{Book, Run};
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

/// A book buffering `runs` (the flush takes them, its mid-run flag and the
/// segments it emits out of the book).
fn book_with<'a>(runs: Vec<Run<'a>>) -> Book<'a> {
    let mut book = Book::new();
    book.runs = runs;
    book
}

/// The `midrun_open` of every statement the book flushed.
fn stamps(book: &Book<'_>) -> Vec<bool> {
    book.segments
        .iter()
        .map(|seg| match seg {
            Segment::Statement { midrun_open, .. } => *midrun_open,
            Segment::Block { .. } => panic!("expected Statement"),
            Segment::AsyncInvalid => panic!("expected Statement"),
        })
        .collect()
}

/// Check a flushed statement segment's fields.
fn check_statement(segment: &Segment<'_>, cmd: &[u8], off: usize, bodies: &[Vec<u8>]) {
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

/// A single-line flush attaches the line's body bytes to the runs in
/// operator order. `line_end` is the newline's position (the segment-flush
/// convention).
#[test]
fn flush_line_attaches_bodies_in_operator_order() {
    //  c a t   < < A  & &  c a t   < < B \n b o d y A \n A \n b o d y B \n B
    //  0       7  8 9 10      17  18                 24 25 26            32 33
    let line = b"cat <<A && cat <<B\nbodyA\nA\nbodyB\nB";
    let mut book = book_with(vec![run(b"cat <<A", 0), run(b"cat <<B", 11)]);
    let resume = flush_line(&mut book, line, 0, 18);
    assert_eq!(book.segments.len(), 2);
    match &book.segments[0] {
        Segment::Statement {
            cmd, off, bodies, ..
        } => {
            assert_eq!(*cmd, b"cat <<A");
            assert_eq!(*off, 0);
            assert_eq!(bodies, &vec![b"bodyA\nA\n".to_vec()]);
        }
        Segment::Block { .. } => panic!("expected Statement"),
        Segment::AsyncInvalid => panic!("expected Statement"),
    }
    match &book.segments[1] {
        Segment::Statement {
            cmd, off, bodies, ..
        } => {
            assert_eq!(*cmd, b"cat <<B");
            assert_eq!(*off, 11);
            assert_eq!(bodies, &vec![b"bodyB\nB".to_vec()]);
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
    let mut book = book_with(vec![run(b"cat <<EOF", 0)]);
    let resume = flush_line(&mut book, line, 0, 20);
    check_statement(&book.segments[0], b"cat <<EOF", 0, &[b"body\nEOF".to_vec()]);
    assert_eq!(resume, 29);
}

/// Runs buffered from an **earlier physical line** — the flush was deferred
/// past the line's newline by a trailing `#` comment (or by a line-opening
/// `&`) — are emitted body-less: matching bodies across lines stays
/// unsupported (owner task #333). This is *not* about a run whose own text
/// spans lines: such a run is flushed on its line and claims its region
/// stream, pinned by `flush_line_a_multi_line_run_claims_its_line_stream`.
#[test]
fn flush_line_multi_line_runs_are_body_less() {
    let line = b"cat <<EOF # c\necho x\nbody\nEOF";
    // The run was buffered on physical line 0 but flushed at line 1's end.
    let mut book = book_with(vec![run(b"cat <<EOF", 0)]);
    let resume = flush_line(&mut book, line, 14, 20);
    check_statement(&book.segments[0], b"cat <<EOF", 0, &[]);
    assert_eq!(resume, 20);
}

/// A run whose **text** spans several physical lines is a single-line run for
/// the flush: a keyword block opened mid-run (`true && while :; do …`, task
/// #232) keeps the block's bytes inside the run and never moves `line_start`,
/// so the flush on the run's own line stamps it with that line's region
/// stream and the body heredoc of the block's body is claimed (task #295,
/// measured bash-equal).
#[test]
fn flush_line_a_multi_line_run_claims_its_line_stream() {
    // The run's text is bytes 0..39 and the line's newline is byte 40, so the
    // line's single region is `bodyT\nT` = bytes 41..48 (the flush's resume).
    let line = b"true && while :; do cat <<T; break; done\nbodyT\nT";
    let mut book = book_with(vec![run(b"true && while :; do cat <<T; break; done", 0)]);
    // `line_end` is the newline that closes `done`: the run's own line.
    let resume = flush_line(&mut book, line, 0, 40);
    check_statement(
        &book.segments[0],
        b"true && while :; do cat <<T; break; done",
        0,
        &[b"bodyT\nT".to_vec()],
    );
    assert_eq!(
        resume, 48,
        "the end of the delimiter line the `<<T` claimed"
    );
}

/// A run with no operators gets no body regions, even when the line has
/// bodies for other runs.
#[test]
fn flush_line_no_operator_run_gets_no_bodies() {
    //  e c h o   x ;   c a t   < < E O F \n b o d y \n E O F
    //  0       5  6    7      15    16  17         21      22     25
    let line = b"echo x; cat <<EOF\nbody\nEOF";
    let mut book = book_with(vec![run(b"echo x", 0), run(b"cat <<EOF", 8)]);
    let resume = flush_line(&mut book, line, 0, 17);
    assert_eq!(book.segments.len(), 2);
    match &book.segments[0] {
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
    match &book.segments[1] {
        Segment::Statement {
            cmd, off, bodies, ..
        } => {
            assert_eq!(*cmd, b"cat <<EOF");
            assert_eq!(*off, 8);
            assert_eq!(bodies, &vec![b"body\nEOF".to_vec()]);
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
    let mut book = book_with(vec![run(b"cat <<A", 0)]);
    let resume = flush_line(&mut book, line, 0, 8);
    check_statement(&book.segments[0], b"cat <<A", 0, &[b"x\nA\n".to_vec()]);
    assert_eq!(resume, 21, "the newline ending the last delimiter line");

    // The line's own flush: only the suffix run is buffered, and its global
    // operator index (`cat <<B` follows one operator written before the `&`)
    // picks region 1 of the same stream.
    let mut book = book_with(vec![run(b"cat <<B", 10)]);
    let resume = flush_line(&mut book, line, 0, 17);
    check_statement(&book.segments[0], b"cat <<B", 10, &[b"y\nB".to_vec()]);
    assert_eq!(resume, 25, "past the line's last region");
}

/// The flush stamps every run it emits with the book's mid-run flag: an
/// unclosed mid-run construct leaves the whole physical line unfinished, so
/// every statement of the flush is the REPL's continuation signal (task #294).
/// The cleared flag is pinned too: a flush is not a continuation by default.
#[test]
fn flush_stamps_the_books_midrun_flag() {
    for flag in [false, true] {
        let mut book = book_with(vec![run(b"echo a", 0), run(b"true && { echo g", 8)]);
        book.midrun_open = flag;
        flush_line(&mut book, b"echo a; true && { echo g", 0, 24);
        assert_eq!(stamps(&book), vec![flag, flag], "flag {flag}");
    }
}

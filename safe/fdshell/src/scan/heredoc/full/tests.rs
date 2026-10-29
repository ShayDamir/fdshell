//! Tests for the full heredoc body regions.

#![allow(clippy::unwrap_used)]

use super::super::op::Operator;
use super::{body_regions_full, contents};
use alloc::vec;
use alloc::vec::Vec;

/// The region spans the body lines plus the delimiter line (including its
/// terminating newline), and the resume index lands on the delimiter line's
/// newline.
#[test]
fn body_regions_full_spans_body_and_delimiter() {
    let line = b"body\nEOF\nnext\n";
    let ops = vec![Operator::new(b"EOF", false, 0)];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(alloc.regions, vec![(0, 9)]);
    assert_eq!(alloc.resume, 8);
    assert_eq!(alloc.found, 1);
}

/// Multiple operators yield sequential regions in operator order.
#[test]
fn body_regions_full_multiple_operators_are_sequential() {
    let line = b"a\nA\nb\nB\n";
    let ops = vec![Operator::new(b"A", false, 0), Operator::new(b"B", false, 0)];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(alloc.regions, vec![(0, 4), (4, 8)]);
    assert_eq!(alloc.resume, 7);
    assert_eq!(alloc.found, 2);
}

/// The line's **first** operator has no delimiter line: the end-of-file shape
/// (task #364) — it claims the rest of the stream like any later missing one,
/// the operator behind it the empty region at the text end, and the resume is
/// the text end so the claimed lines never run as commands. bash reads the
/// body, warns, and runs at rc 0 (measured 2026-10-29).
#[test]
fn body_regions_full_of_a_first_missing_delimiter_claims_the_stream() {
    let line = b"body\nnothing\n";
    let ops = vec![
        Operator::new(b"EOF", false, 0),
        Operator::new(b"X", false, 0),
    ];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!((alloc.regions.len(), alloc.found, alloc.resume), (2, 0, 13));
    assert_eq!(alloc.regions, vec![(0, 13), (13, 13)]);
}

/// A complete operator keeps its region when a **later** operator's delimiter
/// line is missing: the missing one claims the rest of the stream, every
/// operator behind it the empty region at the stream end, and the resume is
/// the text end so the claimed bytes never run as commands (task #275).
#[test]
fn body_regions_full_of_a_later_missing_delimiter_claims_the_stream_tail() {
    let line = b"bodyT\nT\nextra\n";
    let ops = vec![
        Operator::new(b"T", false, 0),
        Operator::new(b"E", false, 0),
        Operator::new(b"G", false, 0),
    ];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(alloc.regions, vec![(0, 8), (8, 14), (14, 14)]);
    assert_eq!(alloc.resume, line.len());
    assert_eq!(alloc.found, 1);
}

/// Every region carries **its own text** and nothing else (task #364): the
/// bytes get a closing newline only when another region follows them (the
/// concatenation rule), and the end-of-file regions — the stream tail and the
/// empty region at the text end — are handed to the parse layer as they are,
/// so it can decide the shape and print the warning itself.
#[test]
fn contents_keeps_the_regions_own_text() {
    let line = b"bodyT\nT\nextra";
    let ops = vec![
        Operator::new(b"T", false, 0),
        Operator::new(b"E", false, 0),
        Operator::new(b"", false, 0),
    ];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(contents(line, &alloc).len(), 3);
    let got = contents(line, &alloc);
    let bodies: Vec<&[u8]> = got.iter().map(|b| b.as_slice()).collect();
    assert_eq!(
        bodies,
        vec![
            b"bodyT\nT\n".as_slice(),
            b"extra\n".as_slice(),
            b"".as_slice()
        ]
    );
}

/// A text that ends mid-line closes the region that is **followed by another**
/// (the concatenation rule): the tail claim's own range is the (unterminated)
/// rest of the stream, and the operator behind it keeps the empty region at the
/// text end — its bytes are empty, and the parse layer decides its heredoc
/// itself (task #364).
#[test]
fn contents_closes_a_region_that_ends_mid_line() {
    let line = b"bodyT\nT";
    let ops = vec![Operator::new(b"T", false, 0), Operator::new(b"E", false, 0)];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(alloc.regions, vec![(0, 7), (8, 7)]);
    let bodies = contents(line, &alloc);
    assert_eq!(bodies.first(), Some(&b"bodyT\nT\n".to_vec()));
    assert_eq!(bodies.get(1), Some(Vec::new()).as_ref());
}

/// A stream that ran out exactly at a delimiter line's newline leaves the
/// missing operator an empty region: its bytes are empty, so the rebuild hands
/// the parse layer a bare command line — the end-of-file shape with an empty
/// body, which bash warns for and runs at rc 0 (task #364).
#[test]
fn contents_of_a_spent_stream_is_empty() {
    let line = b"bodyT\nT\n";
    let ops = vec![Operator::new(b"T", false, 0), Operator::new(b"E", true, 0)];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(alloc.regions, vec![(0, 8), (8, 8)]);
    let bodies = contents(line, &alloc);
    assert_eq!(bodies.get(1), Some(Vec::new()).as_ref());
}

/// The byte layer **adds nothing**: the command line rebuilt as
/// `cmd + "\n" + contents` re-allocates with its operator still unfound, so
/// the end-of-file shape reaches the parse layer exactly once and only it
/// decides (and closes, and warns) — the `<<-` tab-stripped form included
/// (task #364, the no-double-append property of the retired synthesis).
#[test]
fn contents_rebuild_stays_an_end_of_file_shape() {
    let ops = vec![Operator::new(b"T", false, 0), Operator::new(b"E", true, 0)];
    for tail in [b"bodyT\nT\nextra".as_slice(), b"bodyT\nT\n".as_slice()] {
        let mut text = b"cat <<T; cat <<-E\n".to_vec();
        text.extend_from_slice(tail);
        let alloc = body_regions_full(&text, 18, &ops);
        let bodies = contents(&text, &alloc);
        let mut cmd = b"cat <<-E\n".to_vec();
        cmd.extend_from_slice(bodies.get(1).map(|b| b.as_slice()).unwrap_or_default());
        let rebuilt = body_regions_full(&cmd, 9, ops.get(1..).unwrap_or_default());
        assert_eq!(rebuilt.found, 0, "rebuilt bytes={cmd:?}");
    }
}

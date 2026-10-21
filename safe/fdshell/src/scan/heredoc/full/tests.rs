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

/// The line's **first** operator has no delimiter line: the line claims
/// nothing and the resume stays at the stream start — the parse layer owns
/// "this heredoc has no body at all" (task #275's `found == 0` family).
#[test]
fn body_regions_full_of_a_first_missing_delimiter_claim_nothing() {
    let line = b"body\nnothing\n";
    let ops = vec![
        Operator::new(b"EOF", false, 0),
        Operator::new(b"X", false, 0),
    ];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!((alloc.regions.len(), alloc.found, alloc.resume), (0, 0, 0));
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

/// A region past `found` carries its delimiter line **synthesized**: the
/// claimed bytes get a closing newline first when they have none, an empty
/// region carries the bare delimiter line (its body is empty, as bash's
/// end-of-file shape), and an empty delimiter's line is a real blank line.
#[test]
fn contents_synthesizes_the_missing_delimiter_line() {
    let line = b"bodyT\nT\nextra";
    let ops = vec![
        Operator::new(b"T", false, 0),
        Operator::new(b"E", false, 0),
        Operator::new(b"", false, 0),
    ];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(contents(line, &ops, &alloc).len(), 3);
    let got = contents(line, &ops, &alloc);
    let bodies: Vec<&[u8]> = got.iter().map(|b| b.as_slice()).collect();
    assert_eq!(
        bodies,
        vec![
            b"bodyT\nT\n".as_slice(),
            b"extra\nE\n".as_slice(),
            b"\n".as_slice()
        ]
    );
}

/// A text that ends mid-line closes the region's last line, and the missing
/// delimiter line of the tail claim is synthesized: the tail region's own range
/// is the (unterminated) rest of the stream, so its bytes are the stream tail
/// closed to a line plus the delimiter line, and an exhausted stream leaves the
/// operators behind it the empty region.
#[test]
fn contents_closes_a_region_that_ends_mid_line() {
    let line = b"bodyT\nT";
    let ops = vec![Operator::new(b"T", false, 0), Operator::new(b"E", false, 0)];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(alloc.regions, vec![(0, 7), (8, 7)]);
    let bodies = contents(line, &ops, &alloc);
    assert_eq!(bodies.first(), Some(&b"bodyT\nT\n".to_vec()));
    assert_eq!(bodies.get(1), Some(&b"E\n".to_vec()));
}

/// A stream that ran out exactly at a delimiter line's newline leaves the
/// missing operator an empty region: its content is the bare delimiter line,
/// so its heredoc has an empty body (never an unterminated one).
#[test]
fn contents_of_a_spent_stream_is_the_bare_delimiter_line() {
    let line = b"bodyT\nT\n";
    let ops = vec![Operator::new(b"T", false, 0), Operator::new(b"E", true, 0)];
    let alloc = body_regions_full(line, 0, &ops);
    assert_eq!(alloc.regions, vec![(0, 8), (8, 8)]);
    let bodies = contents(line, &ops, &alloc);
    assert_eq!(bodies.get(1), Some(&b"E\n".to_vec()));
}

/// The synthesized line is a **real** delimiter line: the command line rebuilt
/// as `cmd + "\n" + contents` re-allocates with its operator found, so the
/// inner allocation never appends the line a second time — the `<<-`
/// tab-stripped form included.
#[test]
fn contents_of_a_missing_delimiter_reallocates_as_a_complete_heredoc() {
    let ops = vec![Operator::new(b"T", false, 0), Operator::new(b"E", true, 0)];
    for tail in [b"bodyT\nT\nextra".as_slice(), b"bodyT\nT\n".as_slice()] {
        let mut text = b"cat <<T; cat <<-E\n".to_vec();
        text.extend_from_slice(tail);
        let alloc = body_regions_full(&text, 18, &ops);
        let bodies = contents(&text, &ops, &alloc);
        let mut cmd = b"cat <<-E\n".to_vec();
        cmd.extend_from_slice(bodies.get(1).map(|b| b.as_slice()).unwrap_or_default());
        let rebuilt = body_regions_full(&cmd, 9, ops.get(1..).unwrap_or_default());
        assert_eq!(rebuilt.found, 1, "rebuilt bytes={cmd:?}");
    }
}

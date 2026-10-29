//! Tests for the shared block claim (`claim::claimed_bodies`).

#![allow(clippy::unwrap_used)]

use super::claimed_bodies;
use crate::scan::heredoc;
use alloc::vec;
use alloc::vec::Vec;

/// The claim for the block text `line[start..end]`, with the line anchors each
/// block branch computes at its call site: the physical line starts at
/// `line_start` (the call sites pass the run's own start).
fn claim(line: &[u8], start: usize, end: usize, closed: bool) -> (Vec<Vec<u8>>, usize) {
    claim_at(line, start, start, end, closed)
}

/// `claim`, with an explicit physical-line start (the pre-block skip).
fn claim_at(
    line: &[u8],
    line_start: usize,
    start: usize,
    end: usize,
    closed: bool,
) -> (Vec<Vec<u8>>, usize) {
    let line_end = heredoc::line_end_after(line, start);
    let (_, line_resume) = heredoc::line_bodies_for_line(line, start, line_end);
    claimed_bodies(line, line_start, start, end, line_end, line_resume, closed)
}

/// The exclusive span end of a closed block: one past its last `}`.
fn closer_end(line: &[u8]) -> usize {
    line.iter().rposition(|&b| b == b'}').unwrap() + 1
}

/// Append mode: the first region starts at or past the span end, so the
/// contents travel with the block and the span end is unchanged.
#[test]
fn append_mode_returns_the_contents() {
    // A `;`-terminated definer: its region starts past the closing `}`.
    let line = b"f() { cat <<E; }; f\nbody\nE";
    let end = closer_end(line);
    assert_eq!(claim(line, 0, end, true), (vec![b"body\nE".to_vec()], end));
    // The keyword branch's window, one rule over.
    let line = b"if true; then cat <<T; fi\nbodyT\nT";
    let end = line.iter().rposition(|&b| b == b'i').unwrap() + 1;
    assert_eq!(claim(line, 0, end, true), (vec![b"bodyT\nT".to_vec()], end));
}

/// Extension mode: the regions are interleaved with the block's own lines, so
/// no contents travel and the span keeps the block text.
#[test]
fn extension_mode_returns_no_contents_and_the_extended_end() {
    // The multi-line definer: its delimiter lines are inside the span.
    let line = b"f() { cat <<EOF\nin-func\nEOF\n}; f";
    let end = closer_end(line);
    assert_eq!(claim(line, 0, end, true), (Vec::new(), end));
    // A multi-line keyword block whose closer is written **behind** the region:
    // the span extends forward to cover it, forward-only.
    let line = b"if cat <<A; then\necho y\nfi\nbodyA\nA";
    let end = 26;
    let got = claim(line, 0, end, true);
    assert_eq!(got.0, Vec::<Vec<u8>>::new());
    assert_eq!(got.1, line.len(), "the span covers the region's last line");
}

/// A block text with no `<<` operator claims nothing, whatever the stream.
#[test]
fn no_operator_claims_nothing() {
    let line = b"cat <<X; f() { echo hi; }; f; cat <<B\nbodyX\nX\nbodyB\nB";
    let end = 25;
    assert_eq!(claim(line, 9, end, true), (Vec::new(), end));
}

/// An unclosed block claims nothing: its caller reports the missing closer.
#[test]
fn unclosed_block_claims_nothing() {
    let line = b"f() { cat <<E;\nbody";
    let end = line.len();
    assert_eq!(claim(line, 0, end, false), (Vec::new(), end));
}

/// The line's stream claims everything when the **first** delimiter line is
/// missing (task #364): the window keeps its span end — no longer extended —
/// and the contents of the remaining lines travel with it. bash prints
/// `echo after` as the body, warns, and runs at rc 0 (measured 2026-10-29).
#[test]
fn first_missing_delimiter_line_claims_the_stream() {
    let line = b"if cat <<A; then :; fi\necho after";
    let end = 22;
    let got = claim(line, 0, end, true);
    assert_eq!(got.0, vec![b"echo after".to_vec()]);
    assert_eq!(got.1, 22, "the span end stays put");
}

/// The skip is the count of the operators written on the physical line
/// **before** the window, so the definer takes its own region and not the
/// pre-block statement's.
#[test]
fn skip_counts_the_operators_written_before_the_window() {
    let line = b"cat <<X; f() { cat <<E; }; f\nbodyX\nX\nbodyE\nE";
    let end = 25;
    assert_eq!(
        claim_at(line, 0, 9, end, true),
        (vec![b"bodyE\nE".to_vec()], end)
    );
}

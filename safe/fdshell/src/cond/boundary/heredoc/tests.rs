//! Unit pins for the heredoc region walk: a statement's body regions stay
//! opaque (skipped whole), a keyword block's own body regions are scanned for
//! their earliest top-level operator, regions of a `;`-terminated operator
//! line are found on the following lines, a multi-region run skips every
//! region, and a body's unbalanced quotes never fold into the caller's state.

use alloc::vec::Vec;

use crate::cond::boundary::heredoc::{regions, walk};
use crate::cond::boundary::next_boundary;
use crate::scan::ScanState;

/// `next_boundary` from the start of `line`.
fn boundary_of(line: &[u8]) -> (usize, bool) {
    next_boundary(line, 0, &mut ScanState::new())
}

/// `next_boundary` from `i` — the resume point `run_cond_list` uses after it
/// takes the part that ends at the previous boundary.
fn boundary_from(line: &[u8], i: usize) -> (usize, bool) {
    next_boundary(line, i, &mut ScanState::new())
}

/// The regions of the run ending at the newline `nl`, as `next_boundary`
/// computes them.
fn regions_of(line: &[u8], run_start: usize, nl: usize) -> (Vec<(usize, usize)>, usize) {
    regions(line, run_start, nl)
}

/// The regions of the run ending at `line`'s first newline, walked with
/// `block_depth` from index 0; returns the walk's boundary and the index it
/// ended at.
fn walk_of(line: &[u8], block_depth: i32) -> (Option<(usize, bool)>, usize) {
    let nl = line.iter().position(|&b| b == b'\n').unwrap_or(0);
    let (regions, _resume) = regions_of(line, 0, nl);
    let mut i = 0;
    (walk(&regions, block_depth, line, &mut i), i)
}

/// A `;`-terminated operator line finds its body on the following lines: the
/// region window is anchored at the physical line start, so the regions cover
/// the body + delimiter lines, and in block context their `&&` splits.
#[test]
fn semicolon_terminated_operator_line_finds_its_regions() {
    let line = b"if :; then cat <<X ;\nbody && x\nX\nfi";
    // The run starts past the `;` (13) and meets its newline at 20.
    let (regions, resume) = regions_of(&line[..], 13, 20);
    assert_eq!(
        regions.len(),
        1,
        "the `;`-terminated `<<` claims exactly its own region"
    );
    assert_eq!(
        regions.first().copied(),
        Some((21, 33)),
        "body + delimiter lines of the one operator"
    );
    assert_eq!(resume, 32, "the newline terminating the delimiter line");
    let mut i = 13;
    let op = walk(&regions, 1, &line[..], &mut i);
    assert_eq!(
        op,
        Some((26, false)),
        "the body's `&&` splits in block context"
    );
}

/// Statement context (`block_depth == 0`): every region inside the text is
/// skipped whole, so the region's bytes (body lines *and* the delimiter line)
/// never split, and a multi-region run skips all of them.
#[test]
fn statement_regions_are_skipped_whole() {
    let line = b"cat <<A && echo b\nq && r\nX";
    assert_eq!(boundary_of(&line[..]), (8, false), "the pre-body operator");
    // Resuming at the region start: the region is skipped whole — its `&&`
    // (13) is region data — and the scan ends as one part.
    assert_eq!(boundary_from(&line[..], 11), (20, false), "one part");
    // Resuming inside the region: the same, opaque to the region's end.
    assert_eq!(boundary_from(&line[..], 12), (20, false), "one part");
    // Two operators on one run: the run's own `&&` (8) is the boundary, and
    // resuming after it skips the region of the *second* operator whole.
    let line = b"cat <<A && echo b\nq\nA\ntail\nB";
    assert_eq!(boundary_of(&line[..]), (8, false));
    assert_eq!(
        boundary_from(&line[..], 10),
        (28, false),
        "the run's second region is opaque"
    );
    // The walk in statement context skips every region, ending at the last
    // region's newline (24).
    let (op, i) = walk_of(b"cat <<A <<B\nq\nA\nb && c\nB\n", 0);
    assert_eq!(op, None);
    assert_eq!(i, 24, "both regions skipped: last delimiter newline");
}

/// Block context (`block_depth > 0`): the region's earliest top-level `&&` /
/// `||` is the boundary (the pinned body-`&&` split); a region without one is
/// skipped and the scan continues past the regions.
#[test]
fn block_regions_split_on_their_earliest_operator() {
    // `||` is reported as an OR boundary.
    let line = b"if :; then cat <<Q\nz || y\nQ\nfi";
    assert_eq!(boundary_of(&line[..]), (21, true), "the body's `||` (21)");
    // A body with no operator: skipped whole up to its newline (22); the `fi`
    // closes the block and the tail after it splits (26).
    let line = b"if :; then cat <<Q\nz\nQ\nfi && echo tail";
    let (op, i) = walk_of(&line[..], 1);
    assert_eq!(op, None, "the region carries no operator");
    assert_eq!(i, 22, "the region skipped up to its delimiter newline");
    assert_eq!(boundary_of(&line[..]), (26, false), "the tail `&&` (26)");
    // Earliest-first: the second region's operator is reported only when the
    // first region carries none.
    let (op, i) = walk_of(b"cat <<A <<B\nq\nA\nb && c\nB\n", 1);
    assert_eq!(op, Some((18, false)), "the second body's `&&` (18)");
    assert_eq!(i, 15, "the first region was skipped up to its newline");
    // A region that ends at the text end is claimed in block context.
    let line = b"if :; then cat <<X ;\nbody && x\nX\nfi";
    let (op, _i) = walk_of(&line[..], 1);
    assert_eq!(
        op,
        Some((26, false)),
        "the body's `&&` of the block's own run"
    );
}

/// A body legitimately carries unbalanced quotes, backticks and `$( … )`.
/// The region scan uses a fresh state and its bytes are never folded into
/// the caller's: an unbalanced body makes the region scan opaque (no split),
/// and the caller's quote state after the region is untouched.
#[test]
fn region_bytes_never_fold_into_the_callers_state() {
    // Statement context, balanced body: the region (body + delimiter line)
    // is skipped whole; the body's `&&` (10) never splits, and the scan
    // resumes on the delimiter line itself, where the tail's `&&` (17) is.
    let line = b"cat <<Q\nz && y\nQ\n&& echo tail";
    assert_eq!(
        boundary_of(&line[..]),
        (17, false),
        "the operator past the region"
    );
    // Statement context, unbalanced `'` in the body: the region scan is
    // opaque (the tail's `&&` (10) is region data), the region is skipped,
    // and the scan resumes on the delimiter line: the tail's `&&` (17)
    // splits, with the caller's own (clean) state.
    let line = b"cat <<Q\n' && y\nQ\n&& echo tail";
    assert_eq!(boundary_of(&line[..]), (17, false), "the tail's `&&` (17)");
    // Block context, unbalanced `'` in the body: the region scan is opaque,
    // the region is skipped, the `fi` closes the block, and the text after
    // the region carries no operator: one part.
    let line = b"if :; then cat <<Q\n' && y\nQ\nfi";
    assert_eq!(boundary_of(&line[..]), (30, false), "one part");
    // The caller's state survives the region walk unchanged.
    let mut state = ScanState::new();
    next_boundary(b"cat <<Q\n' && y\nQ", 0, &mut state);
    assert!(
        !state.in_quote && !state.in_single,
        "the body folded into the state"
    );
}

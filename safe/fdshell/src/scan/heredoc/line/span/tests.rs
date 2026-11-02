//! Tests for the window-spanning-lines heredoc rules.

use alloc::vec;
use alloc::vec::Vec;

use super::{run_line_bodies, stream_start};

/// A window that stops on the physical line carrying its operator starts its stream
/// just past that line's newline — the same index for the segment flush's edge (the
/// newline's position) and the block-condition extension's edge (just past it).
/// Every single-line window lands here, so the `single_line` rule of
/// `segment/line.rs::flush_line` and the window+skip pair of task #138 are
/// byte-identical to what they were before task #403.
#[test]
fn stream_start_of_a_single_line_window_is_just_past_its_newline() {
    let line = b"cat <<A\nbody\nA\n";
    assert_eq!(stream_start(line, 0, 7), 8, "flush edge: the newline");
    assert_eq!(
        stream_start(line, 0, 8),
        8,
        "extension edge: past the newline"
    );
}

/// A window that runs **past** the physical line carrying its operator starts its
/// stream just after that line, not at the window's own end: each physical line
/// inside the run owns exactly one body stream, so the allocations come out in global
/// per-line operator order (task #403) — the rule the run-level append/extension
/// split below depends on. A window that stops on its operator's line answers with
/// that line's end, exactly as the single-physical-line rule did before task #403.
#[test]
fn stream_start_of_a_window_spanning_lines_starts_past_the_operators_line() {
    let line = b"cat <<A\nbody\nA\n";
    assert_eq!(stream_start(line, 0, 13), 8, "past the operator's line");
    assert_eq!(stream_start(line, 0, line.len()), 8);
    let line = b"head\ncat <<A\nbody\nA\n";
    assert_eq!(stream_start(line, 5, 19), 13, "past the operator's line");
    // The window stops on the operator's own line: the two rules agree.
    assert_eq!(stream_start(line, 5, 13), 13);
}

/// The append half: a region the single definition assigns to bytes **past** the
/// run's end is attached, body lines and the delimiter line's own bytes (the same
/// region the tokenizer blanks with) — the execution layer re-attaches it.
#[test]
fn run_line_bodies_attaches_a_region_past_the_run_end() {
    let line = b"cat <<A\nbody\nA\n";
    assert_eq!(run_line_bodies(line, 7), vec![b"body\nA".to_vec()]);
}

/// The extension half: a region that falls **inside** the run's bytes is already in
/// its text, so nothing is attached and the parse layer re-derives the allocation
/// from the text. One rule with `segment/block/claim.rs`, so the branches cannot
/// drift (task #403).
#[test]
fn run_line_bodies_attaches_nothing_for_a_region_inside_the_run() {
    let line = b"cat <<A\nbody\nA\n";
    assert_eq!(run_line_bodies(line, line.len()), Vec::<Vec<u8>>::new());
}

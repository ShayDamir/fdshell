#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::scan::heredoc::operator_count;

/// A window that claims no region is the plain count, operator-bearing or not:
/// the opaque variant is a subtraction of claimed spans, never a different
/// walk (`sites` is shared unchanged).
#[test]
fn operator_count_opaque_without_a_region_is_the_plain_count() {
    for line in [
        b"echo x <<EOF\n".as_slice(),
        b"cat <<EOF echo\nbody\nEOF\n".as_slice(),
        b"echo a && echo b\n".as_slice(),
    ] {
        for from in [0usize, 5] {
            assert_eq!(
                operator_count_opaque(line, from, line.len()),
                operator_count(line, from, line.len()),
                "{line:?} from {from}"
            );
        }
    }
}

/// A window spanning a body **and** its terminator line counts only the
/// operators written outside them: the terminator's own `<<` is heredoc data
/// (the delimiter word `a <<b` is a word, so its chevron is data), and the
/// global per-line body allocation stays aligned with the token count.
#[test]
fn a_window_spanning_a_body_and_its_terminator_blinds_the_terminator_chevron() {
    let line = b"cat <<'a <<b' <<EOF\nbodyA\nbodyB\na <<b\nEOF\n";
    // Plain: both operators of the command line **plus** the terminator's own.
    assert_eq!(operator_count(line, 0, line.len()), 3);
    assert_eq!(operator_count_opaque(line, 0, line.len()), 2);
    // The terminator line is the last line and the text has no trailing
    // newline: its region ends at `line.len()`, the whole tail is blind.
    let last = b"cat <<'a <<b'\nbody\na <<b";
    assert_eq!(operator_count(last, 0, last.len()), 2);
    assert_eq!(operator_count_opaque(last, 0, last.len()), 1);
}

/// A region claimed by an operator written **before** the window is **not**
/// subtracted: its body starts after the operator line's newline, so it covers
/// the window's own bytes, and subtracting it would blind the count to a block
/// closer written inside that body. The window is `keyword.rs`'s
/// (`segment/block/keyword/tests.rs::skip_is_bounded_to_the_blocks_physical_line`):
/// the block opens on line 2, the pre-block `cat <<X` above it claims the
/// region that swallows the block's line.
#[test]
fn a_region_claimed_before_the_window_is_not_subtracted() {
    let line = b"cat <<X\nif cat <<A; then echo y; fi; cat <<B\nbodyX\nX\nbodyA\nA\nbodyB\nB";
    // `8..36` = the block's command text, `<<A` in it, `fi` right before 36.
    assert_eq!(
        operator_count_opaque(line, 8, 36),
        1,
        "the block's own `<<A`"
    );
    // The same rule one line down: a window entirely inside the swallowed body
    // keeps its operators visible.
    assert_eq!(operator_count_opaque(line, 8, 44), 2, "… and `<<B`");
}

/// A window ending inside a claimed region counts up to its own end: the region
/// bytes behind the window are never visited, so no `<<` of the region is
/// counted and no region byte is scanned twice.
#[test]
fn a_window_ending_inside_a_region_counts_up_to_its_own_end() {
    let line = b"cat <<'a <<b' <<EOF\nbodyA\nbodyB\na <<b\nEOF\n";
    // The window covers the command line only: no region byte is inside it.
    assert_eq!(operator_count_opaque(line, 0, 22), 2);
    // A window that stops inside the first body sees the operators before it.
    assert_eq!(operator_count_opaque(line, 0, 30), 2);
}

/// A pre-block heredoc's region **covers** the following lines, including a
/// block's own command line: the whole-text scan reads the body from the
/// operator line's end and finds the terminator far below, so one region
/// swallows the block. The window decides the subtraction: a window that owns
/// the operator sees the region and stops counting inside it, the block's
/// window on its own line does not subtract it and counts its own operators
/// (the closer and the trailing statement stay visible).
#[test]
fn a_region_covering_the_window_is_subtracted_only_for_its_own_window() {
    // Line 1's `<<X` terminates at the **last** line, so its region covers
    // line 2 whole: `body_regions` yields the single region `8..59`.
    let line = b"cat <<X\nif cat <<A; then echo y; fi; cat <<B\nbodyA\nA\nbodyX\nX";
    assert_eq!(operator_count(line, 0, 46), 3, "plain: `<<X`, `<<A`, `<<B`");
    assert_eq!(
        operator_count_opaque(line, 0, 46),
        1,
        "the window owns `<<X`"
    );
    assert_eq!(
        operator_count_opaque(line, 8, 44),
        2,
        "the block's window keeps `<<A` and `<<B`"
    );
}

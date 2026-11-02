#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

/// The table row's question — what stands **behind** the `for` word, measured at the
/// word's end (task #394). A closed region with a group opener behind it is the
/// arithmetic `for`'s brace body; every other shape keeps the `done` frame.
#[test]
fn the_row_fires_only_for_a_region_followed_by_an_opener() {
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) { echo a; }", 3));
    // The single separator the parse layer's tail rule allows, spaced or glued.
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) ; { echo a; }", 3));
    assert!(opens_a_group(b"for ((i=0;i<2;i++)); { echo a; }", 3));
    // Two separators are bash's rc-2 pair; they keep the parse layer's `do` error.
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) ;; { echo a; }", 3));
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) ; ; { echo a; }", 3));
    // No opener behind the region: the head keeps its `done` frame.
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) do echo a; done", 3));
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) echo a", 3));
    // No whitespace gap is the glued head (task #396), not this row.
    assert!(!opens_a_group(b"for((i=0;i<2;i++)) { echo a; }", 3));
    // A region that never closes hands the word back to the `done` frame.
    assert!(!opens_a_group(b"for ((i=0;i<2;i++ { echo a; }", 3));
    // Not a region at all.
    assert!(!opens_a_group(b"for i in a; do echo a; done", 3));
}

/// The run gate's question, from the whole run and the line behind it (task #394):
/// only a run that is nothing but the head, with the body's opener behind it, opens
/// the block. A plain `do … done` for must not answer true — the row owns it and the
/// `done` frame closes it (the shape the block-tail tests pin).
#[test]
fn the_run_gate_takes_only_a_head_whose_body_is_a_group() {
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) { echo a; }",
        0
    ));
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) ; { echo a; }",
        0
    ));
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)); { echo a; }",
        0
    ));
    // The `do … done` for: the run is the head, the body is not a group.
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) do echo a; done",
        0
    ));
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)); do echo a; done",
        0
    ));
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) ; echo a",
        0
    ));
    // A run carrying more than the head is not the gate's run.
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++)) ;; { echo a; }".as_slice(),
        b"for ((i=0;i<2;i++)) ;; { echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"for((i=0;i<2;i++))".as_slice(),
        b"for((i=0;i<2;i++)) { echo a; }",
        0
    ));
}

/// The run test the group branch and the span start share, called when the caller is
/// already looking at a `{`: the whole text is the head and nothing else.
#[test]
fn the_run_test_wants_the_head_and_nothing_else() {
    assert!(is_run_head(b"for ((i=0;i<2;i++))"));
    assert!(is_run_head(b"for ((i = 0; i < 2; i ++ ))"));
    // Text past the region is body, not head.
    assert!(!is_run_head(b"for ((i=0;i<2;i++)) {"));
    assert!(!is_run_head(b"for ((i=0;i<2;i++)) }"));
    assert!(!is_run_head(b"for ((i=0;i<2;i++)) echo a"));
    assert!(!is_run_head(b"for ((i=0;i<2;i++)) ;"));
    // The glued head (task #396) and a word that is not this construct.
    assert!(!is_run_head(b"for((i=0;i<2;i++))"));
    assert!(!is_run_head(b"for i in a"));
    assert!(!is_run_head(b"for ((i=0;i<2;i++)"));
    assert!(!is_run_head(b"while :; do"));
}

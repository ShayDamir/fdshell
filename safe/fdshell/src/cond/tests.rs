//! Tests for the cond-list body assignment and the group-depth boundary scan.

use super::boundary::next_boundary;
use super::parts::take_bodies;
use crate::scan::ScanState;
use alloc::vec;

/// `next_boundary` from the start of `line`.
fn boundary_of(line: &[u8]) -> (usize, bool) {
    next_boundary(line, 0, &mut ScanState::new())
}

/// `next_boundary` from `i` — the resume point `run_cond_list` uses after it
/// takes the part that ends at the previous boundary.
fn boundary_from(line: &[u8], i: usize) -> (usize, bool) {
    next_boundary(line, i, &mut ScanState::new())
}

/// A `{ …; }` group is opaque: an `&&` inside the body belongs to the body, and
/// the split is the `&&` **after** the closing brace.
#[test]
fn operator_inside_a_group_body_is_not_a_boundary() {
    // `{ echo a && echo b; } && echo c` — the first `&&` (9) is inside the body,
    // the split is the operator at 22.
    assert_eq!(boundary_of(b"{ echo a && echo b; } && echo c"), (22, false));
    // A body `&&` with no operator after the group: no split.
    assert_eq!(boundary_of(b"{ echo a && echo b; }"), (21, false));
    // After `;` the run restarts, so the group after it is counted.
    assert_eq!(boundary_of(b"echo a; { echo b; } && echo c"), (20, false));
    // Two groups as cond-list parts: the split is the `&&` between them.
    assert_eq!(boundary_of(b"{ echo a; } && { echo b; }"), (12, false));
    // A group inside `$( … )` is the substitution's own text: the top-level
    // `&&` after the closing `)` (22) splits.
    assert_eq!(boundary_of(b"echo $( { echo a; } ) && echo c"), (22, false));
}

/// A `}` that closes nothing must clamp the depth at 0: a negative depth would
/// disable the split test for every later `&&` / `||` in the line, swallowing
/// the operator into the command's arguments.
#[test]
fn stray_closer_clamps_the_depth_at_zero() {
    // `echo x} && echo y` — the `}` at 6 closes nothing; the split is the `&&` at 8.
    assert_eq!(boundary_of(b"echo x} && echo y"), (8, false));
    assert_eq!(boundary_of(b"echo -n x} && echo y"), (11, false));
    // A brace-expansion word ends with `}`: the split stays at the `&&`.
    assert_eq!(boundary_of(b"echo {a,b} && echo ok"), (11, false));
    assert_eq!(boundary_of(b"echo {1..3} && echo ok"), (12, false));
    // Several stray closers keep the depth at 0.
    assert_eq!(boundary_of(b"echo }}} && echo y"), (9, false));
}

/// A `{` in an argument position is word data, not a group opener: the depth
/// stays 0 and the `&&` / `||` splits as it does in bash. The same clause-position
/// rule the segment scanner uses (`group_at_run`) governs the counter.
#[test]
fn brace_in_argument_position_is_not_a_group() {
    // `echo { && echo b` — the brace at 5 is an argument; split at the `&&` (7).
    assert_eq!(boundary_of(b"echo { && echo b"), (7, false));
    assert_eq!(boundary_of(b"echo x{ && echo y"), (8, false));
    assert_eq!(boundary_of(b"echo {{ && echo b"), (8, false));
    assert_eq!(boundary_of(b"echo { || echo b"), (7, true));
    // After `;` the run restarts, and the brace is still an argument.
    assert_eq!(boundary_of(b"x=1; echo { && echo $x"), (12, false));
    // A brace glued to a word (brace expansion) is not an opener.
    assert_eq!(boundary_of(b"echo a{b && echo c"), (9, false));
}

/// `take_bodies` assigns the next `n` body bytes to a part with `n` operators.
#[test]
fn take_bodies_assigns_by_operator_count() {
    let line = b"cat <<A && cat <<B";
    let bodies = vec![vec![b'a'], vec![b'b']];
    let mut idx = 0;
    // First part: `cat <<A ` (0..8, the `&&` is at 8..10).
    let first = take_bodies(line, 0, 8, &bodies, &mut idx);
    assert_eq!(first, vec![vec![b'a']]);
    assert_eq!(idx, 1);
    // Second part: ` cat <<B` (10..18, end of line).
    let second = take_bodies(line, 10, 18, &bodies, &mut idx);
    assert_eq!(second, vec![vec![b'b']]);
    assert_eq!(idx, 2);
}

/// A part with no operators gets no body bytes.
#[test]
fn take_bodies_no_operators_gets_none() {
    let line = b"echo hello";
    let bodies = vec![vec![b'a']];
    let mut idx = 0;
    let out = take_bodies(line, 0, 10, &bodies, &mut idx);
    assert!(out.is_empty());
    assert_eq!(idx, 0);
}

// Task #130: `&&` / `||` inside a quoted `$( … )` / `${ … }` / backtick region
// is region content, not a cond-list operator: the split is the operator
// written outside the quotes.
#[test]
fn operator_inside_a_quoted_region_is_not_a_boundary() {
    // `echo "$(printf "a && b")" && echo ok` — inner `&&` at 18, split at 26.
    assert_eq!(
        boundary_of(b"echo \"$(printf \"a && b\")\" && echo ok"),
        (26, false)
    );
    // No operator outside the quotes: the whole line is one cond-list part.
    assert_eq!(boundary_of(b"echo \"$(printf \"a && b\")\""), (25, false));
    // `${ … }` region: `||` at 14 is data, the split is the `||` at 22.
    assert_eq!(
        boundary_of(b"echo \"${u:-\"a || b\"}\" || echo ok"),
        (22, true)
    );
    // Backtick region: `&&` at 17 is data, the split is the `&&` at 25.
    assert_eq!(
        boundary_of(b"echo \"`printf \"a && b\"`\" && echo ok"),
        (25, false)
    );
    // A nested region keeps the same rule: the split is the outer `&&` at 36.
    assert_eq!(
        boundary_of(b"echo \"$(echo \"$(printf \"x && y\")\")\" && echo ok"),
        (36, false)
    );
    // A `;` inside a quoted region does not end the run either: the split after
    // it is the unquoted `&&` at 32.
    assert_eq!(
        boundary_of(b"echo \"$(printf \"a; b\")\"; echo x && echo y"),
        (32, false)
    );
}

// Task #137: keyword-block texts. Inside a block the operator is the block's
// own cond list, so the outer scan finds no boundary; the group rule, the quoted
// regions and the heredoc window keep working around it (the block arm's own
// shapes are pinned in `cond/boundary/block/tests.rs`).
#[test]
fn block_texts_leave_the_outer_scan_no_boundary() {
    // A block inside a group: the split is the group's own trailing `&&` (35).
    assert_eq!(
        boundary_of(b"{ while c && c; do b && c; done; } && echo d"),
        (35, false)
    );
    // A group inside a block condition: the block rule covers the whole text.
    let block = b"while { true; } && { true; }; do echo hi; done";
    assert_eq!(boundary_of(&block[..]), (block.len(), false));
    // A quoted region inside a block body, with an operator written after the
    // block: the split is the trailing `&&` (44) — never the one inside the
    // quotes, and never one inside the block text.
    assert_eq!(
        boundary_of(b"if true; then echo \"$(printf \"a && b\")\"; fi && echo c"),
        (44, false)
    );
    // A multi-line block body: the `&&` on the body's line is inside the block.
    let multi = b"while true; do\n  false && echo a\n  break\ndone";
    assert_eq!(boundary_of(&multi[..]), (multi.len(), false));
}

/// A `'…'` span is opaque (task #141): `echo 'a && b'` is one cond-list part,
/// and the split is the operator written outside the quotes. Bash's split of
/// `echo 'a && b' && echo ok` is the `&&` at 14.
#[test]
fn operator_inside_a_single_quote_is_not_a_boundary() {
    assert_eq!(boundary_of(b"echo 'a && b'"), (13, false), "one part");
    assert_eq!(boundary_of(b"echo 'a && b' && echo ok"), (14, false));
    assert_eq!(boundary_of(b"echo 'a || b' || echo ok"), (14, true));
    // The run resets at a `;`/newline, and a `;` inside a span is not one:
    // `echo 'a; b' && echo c` splits at the `&&` (12), not at the `;`.
    assert_eq!(boundary_of(b"echo 'a; b' && echo c"), (12, false));
}

/// A keyword written inside a `'…'` span opens no block, so the cond-list
/// operators written after it stay top-level (`echo 'a if b' && echo c`).
#[test]
fn keyword_inside_a_single_quote_opens_no_block() {
    assert_eq!(boundary_of(b"echo 'a if b' && echo c"), (14, false));
    assert_eq!(boundary_of(b"echo 'if' fi && echo c"), (13, false));
}

/// Task #232: a run reset (`;` / newline) is also a word reset, so a closing
/// keyword written at the start of a physical line is scored: the block depth
/// returns to 0 and the operator written after the block splits the list,
/// gated by the block's status (bash's rule). The indented variant (the
/// closer's word start was already visible) keeps working.
#[test]
fn line_start_closer_scores_the_blocks_trailing_operator() {
    // `… done && echo x` at a line start: the split is the `&&` (30).
    assert_eq!(
        boundary_of(b"while :; do\necho a\nbreak\ndone && echo x"),
        (30, false)
    );
    // The same block indented: the split is the `&&` (33).
    assert_eq!(
        boundary_of(b"while :; do\n echo a\n break\n done && echo x"),
        (33, false)
    );
    // The `if` twin: `… fi && echo y` at a line start — the split is (24).
    assert_eq!(
        boundary_of(b"if true; then\necho b\nfi && echo y"),
        (24, false)
    );
    // A nested multi-line block in a body (a heredoc-free text): the nested
    // `fi` scores inside the block, its trailing `&&` is the body's own
    // operator, and the whole text stays one part (LESSONS #137 gate 3).
    let multi = b"while :; do\nif true; then echo t; fi && echo x\nbreak\ndone";
    assert_eq!(boundary_of(&multi[..]), (multi.len(), false));
}

/// The word-start bookkeeping after a run reset keeps the gates: a stray
/// closer at a line start clamps the depth at 0 (never hides the operator),
/// and a keyword-shaped word in an argument position is still word data —
/// the command-position rule of `block::word_delta` applies at line starts
/// exactly as it does mid-line.
#[test]
fn line_start_words_keep_the_closer_and_argument_gates() {
    // A stray `done` at a line start clamps at 0: the `&&` (10) splits.
    assert_eq!(boundary_of(b"echo done\n&& echo b"), (10, false));
    // A stray closer at a line start is scored and clamped (11 splits).
    assert_eq!(boundary_of(b"echo if\nfi && echo b"), (11, false));
    // A keyword-shaped argument on the first line stays word data; the second
    // line starts a fresh run and its `&&` (13) splits.
    assert_eq!(boundary_of(b"echo if\ntrue && echo b"), (13, false));
    // A mid-run `<<` with a body: the split is the operator written before
    // the body (8). From the resume (10) the run window is anchored at the
    // physical line start (task #234), so the region is recognized and
    // skipped whole — on this text, which carries its body bytes inline, the
    // body's `&&` no longer splits (one part). Real statement runs hand
    // `run_cond_list` the command bytes only (the regions travel as
    // `bodies`), so their split at 8 is unchanged — pinned by
    // `tests/cond_blocks.rs::mid_run_heredoc_keeps_the_body_inside_the_part`.
    assert_eq!(boundary_of(b"cat <<X && echo b\nq && r\nX"), (8, false));
    assert_eq!(
        boundary_from(b"cat <<X && echo b\nq && r\nX", 10),
        (26, false)
    );
    // The same shape in **block context** (task #234): a block carries its
    // condition bodies inside its own text, the run window is anchored at the
    // physical line start (the `;` reset does not hide the `<<`), and the
    // region walk scans the body region: its `&&` (22) stays the split
    // (`tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part`).
    assert_eq!(
        boundary_of(b"if cat <<X; then\nbody && x\nX\nfi"),
        (22, false)
    );
    // The body-skip arm resumes a fresh run: the operator written on the
    // delimiter line after a body (23) is the split.
    assert_eq!(
        boundary_of(b"echo p; cat <<X\nbody\nX && echo y"),
        (23, false)
    );
}

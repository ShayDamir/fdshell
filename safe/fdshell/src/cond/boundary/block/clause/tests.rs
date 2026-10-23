//! Unit pins for the clause/body mode stack: every opener pushes a clause,
//! `then`/`else`/`do` and a `case` arm's `)` move the innermost block to its
//! body, `elif` and `in` move it back to a clause, a closer pops and a stray
//! closer clamps, an argument-position keyword and a word inside a glued cond
//! pair never transition, and [`Modes::mode_at`] answers the last change at or
//! before the site — with the [`Mode::Clause`] fallback for a site the walk
//! never recorded.

use super::{Mode, Modes};
use crate::keywords::Place;

/// The mode stack after the words of `line` listed in `words` have been scored,
/// in order: each entry is the word's `(byte index, place, block delta)`, the
/// triple the cond-boundary scan hands to [`Modes::scan`].
fn modes_of(line: &[u8], words: &[(usize, Place, i32)]) -> Modes {
    let mut modes = Modes::new();
    for &(i, place, delta) in words {
        modes.scan(line, i, place, delta);
    }
    modes
}

/// The mode of the byte `site` in `line` after the words in `words` — the
/// block's opener and its clause keyword — have been scored.
fn mode_after(line: &[u8], opener: usize, keyword: usize, site: usize) -> Mode {
    modes_of(
        line,
        &[(opener, Place::Command, 1), (keyword, Place::Command, 0)],
    )
    .mode_at(site)
}

/// The `if` at index 0 of a text that opens with `if`: an opener, in command
/// position, scoring `+1`.
const IF: (usize, Place, i32) = (0, Place::Command, 1);
/// The `then` at index 6 of the shared `if :; then …` texts.
const THEN: (usize, Place, i32) = (6, Place::Command, 0);

/// An opener pushes a clause; a closer pops it, and a closer with nothing open
/// clamps at the [`Mode::Clause`] fallback instead of underflowing.
#[test]
fn an_opener_pushes_a_clause_and_a_closer_pops_it() {
    let line = b"if :; then cat <<Q\nz && y\nQ\nfi";
    assert_eq!(
        modes_of(&line[..], &[IF]).mode_at(15),
        Mode::Clause,
        "the block's clause, with nothing said about a body"
    );
    let body = modes_of(&line[..], &[IF, THEN]);
    assert_eq!(body.mode_at(15), Mode::Body, "the body behind `then`");
    let closed = modes_of(&line[..], &[IF, THEN, (28, Place::Command, -1)]);
    assert_eq!(
        closed.mode_at(29),
        Mode::Clause,
        "the popped stack answers with the fallback"
    );
    // A stray closer on an empty stack changes nothing.
    let stray = modes_of(&line[..], &[(28, Place::Command, -1)]);
    assert_eq!(stray.mode_at(15), Mode::Clause, "a stray closer clamps");
}

/// The innermost block's mode wins, and popping reveals the mode of the block
/// that encloses it.
#[test]
fn the_innermost_block_wins() {
    let nested = b"if :; then if :; then cat <<Q\nz && y\nQ\nfi\nfi";
    let inner = modes_of(
        &nested[..],
        &[IF, THEN, (11, Place::Command, 1), (17, Place::Command, 0)],
    );
    assert_eq!(inner.mode_at(26), Mode::Body, "the inner block's body");
    // The inner block opens a clause the site is in: its opener pushed `Clause`
    // and no body keyword of the inner block has been scored yet.
    let inner_clause = modes_of(&nested[..], &[IF, THEN, (11, Place::Command, 1)]);
    assert_eq!(
        inner_clause.mode_at(26),
        Mode::Clause,
        "the inner block's clause, before its `then`"
    );
    // The inner block's closer hands the mode back to the outer body.
    let popped = modes_of(
        &nested[..],
        &[IF, THEN, (11, Place::Command, 1), (39, Place::Command, -1)],
    );
    assert_eq!(popped.mode_at(41), Mode::Body, "the outer body");
}

/// `then`, `else` and `do` each move the innermost block to its body; `elif`
/// and `in` move it back to a clause.
#[test]
fn a_body_keyword_moves_the_innermost_block_to_body() {
    assert_eq!(
        mode_after(b"if :; then cat <<Q\nz && y\nQ\nfi", 0, 6, 15),
        Mode::Body,
        "`then` opens the `if` body"
    );
    assert_eq!(
        mode_after(b"if :; then :; else cat <<Q\nz && y\nQ\nfi", 0, 14, 23),
        Mode::Body,
        "`else` opens the `else` arm's body"
    );
    assert_eq!(
        mode_after(b"while :; do cat <<Q\nz && y\nQ\ndone", 0, 9, 16),
        Mode::Body,
        "`do` opens the loop body"
    );
    assert_eq!(
        mode_after(b"until :; do cat <<Q\nz && y\nQ\ndone", 0, 9, 16),
        Mode::Body,
        "`do` opens the `until` body"
    );
    assert_eq!(
        mode_after(b"if :; then :; elif cat <<Q\nz && y\nQ\nfi", 0, 14, 23),
        Mode::Clause,
        "`elif` opens a fresh condition"
    );
    assert_eq!(
        mode_after(b"if :; then :; else cat <<Q\nz && y\nQ\nfi", 0, 14, 23),
        Mode::Body,
        "the same site behind `else` is a body: the two keywords differ"
    );
}

/// A `case` clause: its word list is a clause, the arm behind its pattern `)`
/// is a body, and `esac` pops the block.
#[test]
fn a_case_clause_closes_at_its_arm_pattern() {
    let list = b"case q in q) cat <<Q\nz && y\nQ\n;;\nesac";
    assert_eq!(
        mode_after(&list[..], 0, 7, 17),
        Mode::Clause,
        "`case` + `in`: the word list stays a clause"
    );
    let arm = modes_of(
        &list[..],
        &[IF_CASE, (7, Place::Command, 0), (10, Place::Command, 0)],
    );
    assert_eq!(arm.mode_at(17), Mode::Body, "the arm's `q)` opens a body");
    let closed = modes_of(
        &list[..],
        &[
            IF_CASE,
            (7, Place::Command, 0),
            (10, Place::Command, 0),
            (33, Place::Command, -1),
        ],
    );
    assert_eq!(
        closed.mode_at(34),
        Mode::Clause,
        "`esac` pops the block, the fallback answers"
    );
}

/// `case` is the opener of the `case` texts above: command position, `+1`.
const IF_CASE: (usize, Place, i32) = (0, Place::Command, 1);

/// The clause gate is `after`'s own: a keyword written as an **argument**
/// (`place == Place::Data`) moves nothing. The `)` row is the one row `after`
/// leaves ungated, and the stack mirrors it (task #430 owns its clause case).
#[test]
fn an_argument_position_keyword_moves_nothing() {
    let line = b"if :; then echo in q) cat <<Q\nz\nQ\nfi";
    let body = modes_of(&line[..], &[IF, THEN]);
    assert_eq!(body.mode_at(26), Mode::Body, "the body at the site");
    let argument = modes_of(&line[..], &[IF, THEN, (16, Place::Data, 0)]);
    assert_eq!(
        argument.mode_at(26),
        Mode::Body,
        "an argument-position `in` moves nothing"
    );
    let clause = modes_of(&line[..], &[IF, THEN, (16, Place::Command, 0)]);
    assert_eq!(
        clause.mode_at(26),
        Mode::Clause,
        "the same word in command position opens the word list"
    );
    let close = modes_of(&line[..], &[IF, (19, Place::Data, 0)]);
    assert_eq!(
        close.mode_at(26),
        Mode::Body,
        "a word ending in `)` opens a body in argument position too"
    );
}

/// A word that starts inside a glued `&&` / `||` pair is the pair's payload
/// position: the scan scores it, the mode stack stays — conservative, the
/// region behind it keeps the mode the block already had.
#[test]
fn a_word_inside_a_glued_pair_never_transitions() {
    let line = b"if :; then a && then cat <<Q\nz\nQ\nfi";
    let body = modes_of(&line[..], &[IF, THEN, (13, Place::Command, 0)]);
    assert_eq!(
        body.mode_at(25),
        Mode::Body,
        "the pair head at 13 is a word start, and moves nothing"
    );
    let clause = modes_of(&line[..], &[IF, (13, Place::Command, 0)]);
    assert_eq!(
        clause.mode_at(25),
        Mode::Clause,
        "in a clause the same word leaves the clause mode"
    );
}

/// `mode_at` answers the **last** change at or before the site: a site written
/// before the block's clause keyword keeps the clause, and the site behind it
/// takes the body.
#[test]
fn mode_at_answers_the_last_change_at_or_before_the_site() {
    let line = b"if cat <<Q; then cat <<R\nz\nQ\nR\nfi";
    let modes = modes_of(&line[..], &[IF, (12, Place::Command, 0)]);
    assert_eq!(modes.mode_at(7), Mode::Clause, "the clause site (7)");
    assert_eq!(modes.mode_at(21), Mode::Body, "the body site (21)");
    assert_eq!(modes.mode_at(0), Mode::Clause, "a site before every change");
    assert_eq!(Modes::new().mode_at(0), Mode::Clause, "the empty stack");
}

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

//! The unit scan: which texts split into how many units (the splitter is the
//! one scanner — these rows pin its answer at the cond granularity), and the
//! dispatch rules the scan feeds: run every unit in order, carry the last
//! unit's status to the test, give each unit the bodies it owns, and let the
//! plain-`&` flag ride on the last unit only (task #557).

use super::{span, units_of};
use crate::state::ShellState;
use alloc::vec;
use alloc::vec::Vec;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn st(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn borrow_state<'a>(cell: &'a ForkCell<ShellState>) -> sys::fork_cell::Ref<'a, ShellState> {
    cell.borrow().unwrap()
}

fn child_test(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            sys::umask::init();
            let saved = sys::umask::get();
            f();
            sys::umask::set(saved);
            sys::exit(42);
        }
        Some(pidfd) => {
            let status = pidfd.wait_pidfd().unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

fn capture_stdout(f: impl FnOnce()) -> Vec<u8> {
    let (out_r, out_w) = sys::pipe::pipe2(0).unwrap();
    match sys::fork_pidfd::fork_pidfd().unwrap().1 {
        None => {
            out_w.export_to(1).unwrap();
            drop(out_w);
            f();
            sys::exit(0);
        }
        Some(pidfd) => {
            drop(out_w);
            let mut out = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = out_r.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                if let Some(part) = chunk.get(..n) {
                    out.extend_from_slice(part);
                }
            }
            match pidfd.wait_pidfd().unwrap() {
                WaitStatus::Exited(0) => {}
                other => panic!("child failed: {}", other.exit_code()),
            }
            out
        }
    }
}

fn run_cond_list(b: &[u8], cell: &ForkCell<ShellState>, background: bool) {
    crate::cond::run_cond_list(&st(b), &[], cell, background).unwrap();
}

fn run_cond_list_bodies(b: &[u8], bodies: &[Vec<u8>], cell: &ForkCell<ShellState>) {
    crate::cond::run_cond_list(&st(b), bodies, cell, false).unwrap();
}

fn run_script(b: &[u8], cell: &ForkCell<ShellState>) {
    crate::script::run_script(&st(b), cell).unwrap();
}

/// The spans of the units a text splits into (offset, length), marks filtered
/// out exactly as the dispatcher does.
fn spans(text: &[u8]) -> Vec<(usize, usize)> {
    units_of(text).iter().map(|seg| span(seg)).collect()
}

/// A plain separator at top level is the split the cond layer learns through
/// the scanner: two units, each with its exact span (the numbers pin the
/// dispatcher's own offset arithmetic).
#[test]
fn a_plain_separator_splits_the_text_into_two_units() {
    assert_eq!(spans(b"true; false"), vec![(0, 4), (6, 5)]);
    // The task's headline shape: the definition is claimed as a block (task
    // #235's definer opener), the call is the second unit.
    assert_eq!(spans(b"g() { false; }; g"), vec![(0, 14), (16, 1)]);
    // A group claimed at a run start is a block unit, and the run behind the
    // separator is the second.
    assert_eq!(spans(b"{ echo a; }; true"), vec![(0, 11), (13, 4)]);
}

/// A separator inside a quoted word, comment text, a here-doc region or a
/// keyword block is **data**: the scan yields one unit and the walk sees the
/// text as it does today.
#[test]
fn separators_inside_quotes_comments_regions_and_blocks_stay_data() {
    assert_eq!(spans(b"echo \"a ; b\""), vec![(0, 12)]);
    assert_eq!(spans(b"echo a # b; c"), vec![(0, 6)]);
    assert_eq!(spans(b"cat <<Q\nb ; c\nQ"), vec![(0, 7)]);
    assert_eq!(spans(b"if :; then :; fi"), vec![(0, 16)]);
}

/// POSIX #2.9's AND-IF-TERMINATOR (task #492): a newline **behind** a top-level
/// `&&`/`||` is part of the operator, so the run stays one unit; a plain
/// newline is a separator and splits.
#[test]
fn the_operator_newline_join_keeps_one_unit_and_a_plain_newline_splits() {
    assert_eq!(spans(b"false ||\ntrue"), vec![(0, 13)]);
    assert_eq!(spans(b"false\ntrue"), vec![(0, 5), (6, 4)]);
}

/// The dispatch: every unit runs, in order, and the **last** unit's status is
/// what reaches the test — bash's `compound_list`. The order row and both
/// status rows flip from the pre-#557 first-command answer to bash's value.
#[test]
fn every_unit_runs_in_order_and_the_last_status_reaches_the_test() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_cond_list(b"builtin echo a; builtin echo b", &cell, false);
    });
    assert_eq!(out, b"a\nb\n");
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"builtin false; builtin true", &cell, false);
        assert_eq!(borrow_state(&cell).last_status.exit_code(), 0);
    });
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"builtin true; builtin false", &cell, false);
        assert_eq!(borrow_state(&cell).last_status.exit_code(), 1);
    });
}

/// Each unit gets the bodies it owns: the caller's list is shared in scan
/// order, so the first `<<` unit gets the first body and the second unit the
/// second (the global per-line operator order, LESSONS). Both commands run
/// here, unlike in the operator-only walk.
#[test]
fn each_unit_gets_the_bodies_it_owns() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        let bodies = [b"q-body\nQ".to_vec(), b"r-body\nR".to_vec()];
        run_cond_list_bodies(b"cat <<Q; cat <<R", &bodies, &cell);
    });
    assert_eq!(out, b"q-body\nr-body\n");
}

/// The plain-`&` flag rides on the **last** unit only: the first unit runs in
/// the foreground (its `cd` moves this process), the flagged one is the single
/// job. Without the flag there is no job at all (task #326's rule at the new
/// granularity).
#[test]
fn the_background_flag_rides_on_the_last_unit_only() {
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"cd /tmp; builtin true", &cell, true);
        assert_eq!(borrow_state(&cell).jobs.anon_len(), 1);
        assert_eq!(
            std::env::current_dir().unwrap(),
            std::path::Path::new("/tmp")
        );
    });
    child_test(|| {
        let cell = make_cell();
        run_cond_list(b"cd /tmp; builtin true", &cell, false);
        assert_eq!(borrow_state(&cell).jobs.anon_len(), 0);
    });
}

/// Loop control raised by any unit leaves the condition at once: the `break`
/// runs, propagates, and the **second** unit behind it never runs (the
/// existing early-return contract at the new granularity — task #205's rule).
#[test]
fn loop_control_in_any_unit_leaves_the_condition_at_once() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_script(
            b"while :; do\nif break; builtin echo never; then :; fi\ndone\necho tail",
            &cell,
        );
    });
    assert_eq!(out, b"tail\n");
}

/// The dispatcher acts on statement and block units only: the scan's marks
/// (here the stray `;;`, task #423) are the pre-passes' business on the whole
/// text, so the units dispatcher reports nothing itself and runs the units
/// the scan produced (the `x ;; y` scan yields two units around the mark).
#[test]
fn the_dispatcher_acts_on_units_not_marks() {
    // `x ;; y`: the mark sits between the two runs; the dispatcher's filter
    // keeps exactly the two units (the mark is not a unit and gets no unit
    // span — the `Report` row for this shape lives in `tests/semi_semi.rs`).
    assert_eq!(spans(b"x ;; y"), vec![(0, 1), (5, 1)]);
    assert_eq!(spans(b"x ; y ;; z"), vec![(0, 1), (4, 1), (9, 1)]);
}

/// A block unit whose span **opens with whitespace** (the scanner hands the run
/// from just past the separator, braces included) is trimmed by the exact
/// offset in `unit_text`: the block's command bytes start on the keyword, not on
/// the separator byte behind the space. bash value measured 2026-11-13 (5.3.9):
/// `BODY` printed, rc 1 — every unit runs and the last gates the test.
#[test]
fn a_block_unit_span_opening_with_whitespace_is_trimmed() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        run_cond_list(
            b"builtin true; if builtin true; then builtin echo BODY; fi; builtin false",
            &cell,
            false,
        );
        assert_eq!(borrow_state(&cell).last_status.exit_code(), 1);
    });
    assert_eq!(out, b"BODY\n");
}

/// The two body sources **mixed** in one run, and the cursor `own_bodies` keeps
/// over them: the first unit's region is inside the scanned text (so the scan
/// assigned it the body), the second unit's travels in the caller's list — which
/// is the global per-line order (LESSONS: "A heredoc body is a property of the
/// logical line, not of a contiguous span") and therefore holds **both** bodies,
/// the one the scan already handed out included. Each unit must get its own: a
/// unit served from the scanner without consuming its share of the list hands the
/// first body to the second unit too. The order is bash's measured one — `q-body`
/// then `r-body` (the file shapes of this family are pinned in
/// `tests/cond_blocks.rs`).
#[test]
fn a_mix_of_scanner_assigned_and_listed_bodies_is_served_once() {
    let out = capture_stdout(|| {
        let cell = make_cell();
        let bodies = [b"q-body\nQ".to_vec(), b"r-body\nR".to_vec()];
        run_cond_list_bodies(b"cat <<Q\nq-body\nQ\ncat <<R", &bodies, &cell);
    });
    assert_eq!(out, b"q-body\nr-body\n");
}

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::ParsedLine;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// Parse a `while`/`until` statement into its `LoopBlock` (panics if it is not one).
fn loop_block(line: &[u8]) -> LoopBlock {
    match crate::parse::parse(&stext(line)).unwrap() {
        ParsedLine::While(block) | ParsedLine::Until(block) => block,
        _ => panic!("not a loop statement: {line:?}"),
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

/// Seed the state with in-process statements (`i=0`, …) before running the loop.
fn seed(cell: &ForkCell<ShellState>, setup: &[u8]) {
    crate::script::run_script(&stext(setup), cell).unwrap();
}

fn status(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow().unwrap().last_status.exit_code()
}

fn depth(cell: &ForkCell<ShellState>) -> u32 {
    cell.borrow().unwrap().loop_depth
}

fn var(cell: &ForkCell<ShellState>, name: &str) -> Option<sys::ShortCStr> {
    let key = sys::ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap();
    cell.borrow()
        .unwrap()
        .strings
        .get::<sys::ShortCStr>(&key)
        .map(|s| s.value.clone())
}

/// A `break` in the condition ends the loop before its body ever runs, and the
/// loop reports status 0 (bash: `while if true; then break; fi; do …; done` → rc 0).
#[test]
fn break_in_the_condition_ends_the_loop() {
    let cell = make_cell();
    let block = loop_block(b"while if :; then break; fi; do result=1; done");
    assert_eq!(run_loop(&block, true, &cell).unwrap(), None);
    assert!(var(&cell, "result").is_none());
    assert_eq!(status(&cell), 0);
    assert_eq!(depth(&cell), 0);
}

/// `break 2` in a condition of a nested loop hands the remainder to the outer
/// level (`for` outside = the preset depth) and `in_loop` restores the depth.
#[test]
fn break_2_in_the_condition_propagates_the_remainder() {
    let cell = make_cell();
    cell.borrow_mut().unwrap().loop_depth = 1;
    let block = loop_block(b"while break 2; do result=1; done");
    assert_eq!(
        run_loop(&block, true, &cell).unwrap(),
        Some(LoopControl::Break(1))
    );
    assert_eq!(depth(&cell), 1);
    assert!(var(&cell, "result").is_none());
}

/// A `continue` in a condition is the loop's next iteration: the body is skipped
/// and the condition is re-tested (bash hangs on `while continue; do …; done`;
/// this shape ends because the re-tested condition stops raising `continue`).
#[test]
fn continue_in_the_condition_re_tests_the_condition() {
    let cell = make_cell();
    seed(&cell, b"i=0");
    let block = loop_block(
        b"while i=$((i+1)) && (( i < 3 )) && if :; then continue; fi; do result=1; done",
    );
    assert_eq!(run_loop(&block, true, &cell).unwrap(), None);
    assert!(var(&cell, "result").is_none());
    assert!(var(&cell, "i").is_some_and(|v| v.eq_bytes(b"3")));
}

/// `return` in a condition leaves the loop whole, status included (bash:
/// `f() { while if true; then return 3; fi; do :; done; echo afterloop; }; f` → rc 3).
#[test]
fn return_in_the_condition_propagates_out_of_the_loop() {
    let cell = make_cell();
    let block = loop_block(b"while if :; then return 3; fi; do result=1; done");
    assert_eq!(
        run_loop(&block, true, &cell).unwrap(),
        Some(LoopControl::Return)
    );
    assert!(var(&cell, "result").is_none());
    assert_eq!(status(&cell), 3);
    assert_eq!(depth(&cell), 0);
}

/// The `until` twin: a condition `break 2` in depth 2 exits the `until` and hands
/// `Break(1)` to the outer loop. Before the fix the condition's control was
/// dropped, so this call returned `None`.
#[test]
fn break_2_in_an_until_condition_propagates_the_remainder() {
    let cell = make_cell();
    cell.borrow_mut().unwrap().loop_depth = 1;
    let block = loop_block(b"until if :; then break 2; fi; do result=1; done");
    assert_eq!(
        run_loop(&block, false, &cell).unwrap(),
        Some(LoopControl::Break(1))
    );
    assert_eq!(depth(&cell), 1);
    assert!(var(&cell, "result").is_none());
}

/// A `break` in an `until` condition ends it (bash: `until if true; then break; fi;
/// do echo body; done` prints nothing and exits 0).
#[test]
fn break_in_an_until_condition_ends_the_loop() {
    let cell = make_cell();
    let block = loop_block(b"until if :; then break; fi; do result=1; done");
    assert_eq!(run_loop(&block, false, &cell).unwrap(), None);
    assert!(var(&cell, "result").is_none());
    assert_eq!(status(&cell), 0);
}

/// The body arms keep consuming exactly one level through [`at_level`]:
/// `break 2` in depth 2 propagates `Break(1)`, a bare `break` ends this loop.
#[test]
fn body_arms_still_consume_one_level() {
    let cell = make_cell();
    cell.borrow_mut().unwrap().loop_depth = 1;
    let block = loop_block(b"while :; do break 2; done");
    assert_eq!(
        run_loop(&block, true, &cell).unwrap(),
        Some(LoopControl::Break(1))
    );
    let cell = make_cell();
    let block = loop_block(b"while :; do break; done");
    assert_eq!(run_loop(&block, true, &cell).unwrap(), None);
}

/// A `continue` in the body skips the rest of that iteration and re-tests the
/// condition; the loop then runs its last iteration and ends.
#[test]
fn continue_in_the_body_skips_the_rest_of_the_iteration() {
    let cell = make_cell();
    seed(&cell, b"i=0");
    let block = loop_block(
        b"while i=$((i+1)); do if (( i < 2 )); then continue; fi; result=$i; break; done",
    );
    assert_eq!(run_loop(&block, true, &cell).unwrap(), None);
    assert!(var(&cell, "result").is_some_and(|v| v.eq_bytes(b"2")));
}

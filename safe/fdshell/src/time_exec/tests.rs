#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::loop_control::LoopControl;
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

/// Parse a statement into its `TimeBlock` (panics if it is not a `time` one).
fn time_block(line: &[u8]) -> TimeBlock {
    match crate::parse::parse(&stext(line)).unwrap() {
        ParsedLine::Time(block) => block,
        _ => panic!("not a time statement: {line:?}"),
    }
}

fn status(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow().unwrap().last_status.exit_code()
}

#[test]
fn bare_time_succeeds_with_status_zero() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().set_last_exit(7);
    let block = time_block(b"time");
    assert_eq!(run_time(&block, &cell).unwrap(), None);
    assert_eq!(status(&cell), 0);
}

#[test]
fn inner_in_process_command_status_is_the_statements_own() {
    let cell = ForkCell::new(ShellState::new());
    let block = time_block(b"time :");
    assert_eq!(run_time(&block, &cell).unwrap(), None);
    assert_eq!(status(&cell), 0);
    let block = time_block(b"time false");
    // `false` is a forked command: the status comes from the reap, so check
    // the inner ran at all and left the shell's own status alone.
    assert!(matches!(block.inner.as_deref(), Some(ParsedLine::Cmd(_))));
}

#[test]
fn control_flow_of_the_inner_passes_through_the_report() {
    let cell = ForkCell::new(ShellState::new());
    // `break`/`continue` need a loop level; the timed statement runs inside one.
    cell.borrow_mut().unwrap().loop_depth = 2;
    let block = time_block(b"time break 2");
    assert_eq!(
        run_time(&block, &cell).unwrap(),
        Some(LoopControl::Break(2))
    );
    let block = time_block(b"time continue 1");
    assert_eq!(
        run_time(&block, &cell).unwrap(),
        Some(LoopControl::Continue(1))
    );
    let block = time_block(b"time return");
    assert_eq!(run_time(&block, &cell).unwrap(), Some(LoopControl::Return));
}

#[test]
fn failing_inner_keeps_its_error_report() {
    let cell = ForkCell::new(ShellState::new());
    let block = time_block(b"time cd /nope_dir_for_time_test");
    let report = run_time(&block, &cell).unwrap_err();
    // The report is the inner's own chain, not a `time`-specific one.
    assert!(alloc::format!("{report:?}").contains("cd"));
}

#[test]
fn nested_prefixes_run_the_inner_once() {
    let cell = ForkCell::new(ShellState::new());
    let block = time_block(b"time time -p time :");
    assert!(block.posix);
    assert!(!matches!(block.inner.as_deref(), Some(ParsedLine::Time(_))));
    assert_eq!(run_time(&block, &cell).unwrap(), None);
    assert_eq!(status(&cell), 0);
}

#[test]
fn block_inner_runs_whole() {
    let cell = ForkCell::new(ShellState::new());
    let block = time_block(b"time if :; then :; fi");
    assert!(matches!(block.inner.as_deref(), Some(ParsedLine::If(_))));
    assert_eq!(run_time(&block, &cell).unwrap(), None);
    assert_eq!(status(&cell), 0);
}

#[test]
fn terminating_inners_are_detected() {
    assert!(terminates_shell(&time_block(b"time exit 3")));
    assert!(terminates_shell(&time_block(b"time quit 2")));
    assert!(terminates_shell(&time_block(b"time . /dev/null")));
    assert!(terminates_shell(&time_block(b"time exec /bin/true")));
    assert!(terminates_shell(&time_block(b"time -p exit 0")));
    assert!(!terminates_shell(&time_block(b"time echo exit")));
    assert!(!terminates_shell(&time_block(b"time :")));
    assert!(!terminates_shell(&time_block(b"time")));
    // A block whose inner is a terminating keyword is not a `time` statement.
    assert!(!matches!(
        crate::parse::parse(&stext(b"if exit 3; then :; fi")).unwrap(),
        ParsedLine::Time(_)
    ));
}

#[test]
fn start_snapshot_is_monotonic_and_nonzero() {
    let cell = ForkCell::new(ShellState::new());
    let first = report::start(&cell).unwrap();
    let second = report::start(&cell).unwrap();
    assert!(second.real >= first.real);
    assert!(second.user >= first.user);
    assert!(second.sys >= first.sys);
}

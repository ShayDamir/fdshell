#![allow(clippy::unwrap_used)]
use super::*;
use crate::error::cmd::CmdError;
use error_stack::Report;

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn depth(cell: &ForkCell<ShellState>) -> u32 {
    cell.borrow().unwrap().loop_depth
}

#[test]
fn in_loop_increments_depth_for_the_body() {
    let cell = make_cell();
    in_loop(&cell, || {
        assert_eq!(depth(&cell), 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(depth(&cell), 0);
}

#[test]
fn in_loop_restores_depth_on_error() {
    let cell = make_cell();
    let result: Result<(), Report<CmdError>> =
        in_loop(&cell, || Err(Report::new(CmdError::Invalid)));
    assert!(result.is_err());
    assert_eq!(depth(&cell), 0);
}

#[test]
fn in_loop_restores_depth_after_break() {
    let cell = make_cell();
    let control = in_loop(&cell, || Ok(Some(LoopControl::Break(1)))).unwrap();
    assert!(matches!(control, Some(LoopControl::Break(1))));
    assert_eq!(depth(&cell), 0);
}

#[test]
fn in_loop_nests() {
    let cell = make_cell();
    in_loop(&cell, || {
        in_loop(&cell, || {
            assert_eq!(depth(&cell), 2);
            Ok(())
        })
        .unwrap();
        assert_eq!(depth(&cell), 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(depth(&cell), 0);
}

#[test]
fn consume_one_level_of_break() {
    assert_eq!(consume(LoopControl::Break(2)), Some(LoopControl::Break(1)));
    assert_eq!(consume(LoopControl::Break(1)), None);
}

#[test]
fn consume_one_level_of_continue() {
    assert_eq!(
        consume(LoopControl::Continue(3)),
        Some(LoopControl::Continue(2))
    );
    assert_eq!(consume(LoopControl::Continue(1)), None);
}

#[test]
fn consume_leaves_return_and_exit_untouched() {
    assert!(matches!(
        consume(LoopControl::Return),
        Some(LoopControl::Return)
    ));
    assert!(matches!(
        consume(LoopControl::Exit),
        Some(LoopControl::Exit)
    ));
}

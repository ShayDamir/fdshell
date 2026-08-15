#![allow(clippy::unwrap_used)]
use super::*;
use crate::error::cmd::CmdError;

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

#[test]
fn deeper_restores_depth_on_error() {
    let cell = make_cell();
    let result: Result<(), Report<CmdError>> = deeper(&cell, CmdError::NestingTooDeep, || {
        Err(Report::new(CmdError::Invalid))
    });
    assert!(result.is_err());
    assert_eq!(cell.borrow().unwrap().nesting, 0);
}

#[test]
fn deeper_restores_depth_on_success() {
    let cell = make_cell();
    let value = deeper(&cell, CmdError::NestingTooDeep, || {
        let inner = deeper(&cell, CmdError::NestingTooDeep, || {
            assert_eq!(cell.borrow().unwrap().nesting, 2);
            Ok(7)
        })
        .unwrap();
        Ok(inner)
    })
    .unwrap();
    assert_eq!(value, 7);
    assert_eq!(cell.borrow().unwrap().nesting, 0);
}

#[test]
fn deeper_enforces_limit() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.nesting = MAX_NESTING;
    }
    let result = deeper(&cell, CmdError::NestingTooDeep, || Ok(1));
    assert!(matches!(
        result.unwrap_err().current_context(),
        CmdError::NestingTooDeep
    ));
    assert_eq!(cell.borrow().unwrap().nesting, MAX_NESTING);
}

#![allow(clippy::unwrap_used)]

use crate::state::ShellState;
use sys::fork_cell::ForkCell;

#[test]
fn eof_exits_when_ignoreeof_off() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().options &= !crate::options::IGNOREEOF;
    assert!(!super::eof_continues(&cell).unwrap());
}

#[test]
fn eof_continues_when_ignoreeof_on() {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().options |= crate::options::IGNOREEOF;
    assert!(super::eof_continues(&cell).unwrap());
}

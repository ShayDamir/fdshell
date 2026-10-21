#![allow(clippy::unwrap_used)]

use crate::options::IGNOREEOF;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// A shell state with the given `ignoreeof` bit and interactive flag, wrapped
/// in the `ForkCell` the EOF policy reads (task #291's 2 × 2 matrix).
fn cell_with(ignoreeof: bool, interactive: bool) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        if ignoreeof {
            state.options |= IGNOREEOF;
        } else {
            state.options &= !IGNOREEOF;
        }
        state.interactive = interactive;
    }
    cell
}

#[test]
fn eof_off_exits_on_terminal_and_pipe() {
    // `ignoreeof` off: exit at EOF on a terminal and over a pipe alike.
    assert!(!super::line::eof_continues(&cell_with(false, true)).unwrap());
    assert!(!super::line::eof_continues(&cell_with(false, false)).unwrap());
}

#[test]
fn eof_on_terminal_keeps_reading() {
    // `ignoreeof` on, stdin is a terminal: the hint is printed and the shell
    // keeps reading (a tty delivers EOF as an event, then blocks).
    assert!(super::line::eof_continues(&cell_with(true, true)).unwrap());
}

#[test]
fn eof_on_pipe_exits_after_the_hint() {
    // The task #291 regression pin: `ignoreeof` on over a pipe (the default
    // `interactive: false`) must not keep reading — a pipe's EOF is sticky, so
    // continuing would print the hint forever.
    assert!(!super::line::eof_continues(&cell_with(true, false)).unwrap());
}

#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::ShellState;
use alloc::vec;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

fn make_cmdline() -> crate::parse::CommandLine {
    crate::parse::CommandLine {
        prefix: crate::parse::BuiltinPrefix::None,
        command: ShortCStr::from(c":"),
        command_mask: vec![],
        env_assigns: vec![],
        args: vec![],
        args_mask: vec![],
        args_quoted: vec![],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

#[test]
fn run_colon_is_handled_and_sets_zero() {
    let cell = ForkCell::new(ShellState::new());
    // Seed a nonzero status so the assertion below is not tautological:
    // `:` must reset a failed status to 0.
    cell.borrow_mut().unwrap().set_last_exit(1);
    let cmdline = make_cmdline();
    assert!(run_colon(b":", &cmdline, &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(state.last_status.exit_code(), 0);
}

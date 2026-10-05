#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::ShellState;
use alloc::vec;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

#[test]
fn centis_formats_microseconds() {
    assert_eq!(centis(0), "0.00");
    assert_eq!(centis(10_000), "1.00");
    assert_eq!(centis(12_345), "1.23");
    assert_eq!(centis(1_234_567), "123.45");
}

fn make_cmdline() -> crate::parse::CommandLine {
    crate::parse::CommandLine {
        prefix: crate::parse::BuiltinPrefix::None,
        command: ShortCStr::from(c"times"),
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
fn run_times_is_handled_and_sets_zero() {
    let cell = ForkCell::new(ShellState::new());
    let cmdline = make_cmdline();
    assert!(run_times(b"times", &cmdline, &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(state.last_status.exit_code(), 0);
}

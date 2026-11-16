#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::ShellState;
use alloc::vec;
use sys::fork_cell::ForkCell;

fn make_cmdline() -> crate::parse::CommandLine {
    crate::parse::CommandLine {
        prefix: crate::parse::BuiltinPrefix::None,
        command: ShortCStr::from(c"getopts"),
        command_mask: vec![],
        command_span: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: vec![c"a".into(), c"o".into()],
        args_mask: vec![vec![], vec![]],
        args_span: vec![vec![], vec![]],
        args_quoted: vec![false, false],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

/// The counter's default is **1** — the value bash 5.3.9 measures at startup, and
/// the value `unset OPTIND` falls back to (task #589).
#[test]
fn the_default_counter_is_one() {
    assert_eq!(DEFAULT_OPTIND, 1);
}

/// Only an **exact** decimal moves the counter; a value with no digits at all
/// leaves it where it was (bash measures `OPTIND=abc` as "start the scan at 1"
/// while `$OPTIND` prints `abc`). The arithmetic-evaluator forms are #590's.
#[test]
fn only_an_exact_parse_moves_the_counter() {
    assert_eq!(counter(&ShortCStr::from(c"3"), DEFAULT_OPTIND), 3);
    assert_eq!(counter(&ShortCStr::from(c"abc"), 4), 4);
    assert_eq!(counter(&ShortCStr::from(c""), 4), 4);
}

/// The stub **claims** the command — `true` means handled, so `run/parent.rs`
/// never sends `getopts` down the forked launch path — and reports success.
/// **#590 moves this pin**: bash's `getopts a o` with no operands exits 1.
#[test]
fn run_getopts_is_handled_and_reports_success() {
    let cell = ForkCell::new(ShellState::new());
    // Seed a nonzero status so the assertion below is not tautological (the
    // `colon` tests' shape): the handler must reset a failed status to 0.
    cell.borrow_mut().unwrap().set_last_exit(1);
    let cmdline = make_cmdline();
    assert!(run_getopts(b"getopts a o", &cmdline, &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(state.last_status.exit_code(), 0, "the stub's status");
    assert_eq!(state.optind, DEFAULT_OPTIND, "the stub does not move it");
}

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

/// `:` is an intercept, so a capture stays rejected (`CapturesNotSupported`):
/// a capture needs a forked child to send the fd over the shell socket.
#[test]
fn captures_are_rejected() {
    let cell = ForkCell::new(ShellState::new());
    let mut cmdline = make_cmdline();
    cmdline.captures = vec![crate::capture::Capture {
        var: c"x".into(),
        tag: None,
        force: false,
        cap: None,
        set_at: sys::Position::new(1, 1),
    }];
    let report = run_colon(b": %>%x", &cmdline, &cell).unwrap_err();
    assert!(matches!(
        report.current_context(),
        crate::error::cmd::CmdError::CapturesNotSupported { .. }
    ));
}

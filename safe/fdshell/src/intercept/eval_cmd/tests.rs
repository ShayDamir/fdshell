#![allow(clippy::unwrap_used)]
use super::{join_space, run_eval};
use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

#[test]
fn join_space_separates_args_with_single_spaces() {
    let args = [c"a".into(), c"b".into(), c"c".into()];
    assert!(join_space(&args).eq_bytes(b"a b c"));
}

#[test]
fn join_space_single_arg_has_no_separator() {
    let args = [c"solo".into()];
    assert!(join_space(&args).eq_bytes(b"solo"));
}

fn make_cmdline(command: &[u8], args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(command.to_vec()).unwrap(),
        args: args_vec,
        args_mask: vec![vec![]; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn text(bytes: &[u8]) -> sys::ScriptText {
    sys::ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        sys::Position::new(1, 1),
        sys::Origin::Shell,
    )
}

#[test]
fn run_eval_counts_toward_nesting_limit() {
    let cell = make_cell();
    let cmdline = make_cmdline(b"eval", &["x=1"]);
    let text = text(b"eval");

    // One level below the cap: entering the eval level succeeds.
    {
        let mut state = cell.borrow_mut().unwrap();
        state.nesting = crate::nest::MAX_NESTING - 1;
    }
    assert!(run_eval(b"eval", &cmdline, &text, &cell).unwrap().is_none());
    assert_eq!(cell.borrow().unwrap().nesting, crate::nest::MAX_NESTING - 1);

    // At the cap: entering the eval level fails with NestingTooDeep.
    {
        let mut state = cell.borrow_mut().unwrap();
        state.nesting = crate::nest::MAX_NESTING;
    }
    let e = run_eval(b"eval", &cmdline, &text, &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::NestingTooDeep));
    assert_eq!(cell.borrow().unwrap().nesting, crate::nest::MAX_NESTING);
}

#![allow(clippy::unwrap_used)]
use alloc::vec;

use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText, ShortCStr};

use crate::parse::CommandLine;
use crate::state::ShellState;

fn text(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn cmdline(command: &[u8]) -> CommandLine {
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(command.to_vec()).unwrap(),
        args: vec![],
        args_fq: vec![],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

#[test]
fn unknown_command_is_not_intercepted() {
    let cell = make_cell();
    let r = crate::function_call::try_call(&text(b"ls"), &cmdline(b"ls"), &cell).unwrap();
    assert!(r.is_none());
}

#[test]
fn defined_function_is_intercepted_and_runs_body() {
    let cell = make_cell();
    crate::script::run_script(&text(b"f() { v=hi; }"), &cell).unwrap();
    let r = crate::function_call::try_call(&text(b"f"), &cmdline(b"f"), &cell).unwrap();
    assert!(r.is_some());
    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"v".into())
            .map(|s| &s.value),
        Some(&c"hi".into())
    );
}

#[test]
fn builtin_prefix_bypasses_function() {
    let cell = make_cell();
    crate::script::run_script(&text(b"f() { v=hi; }"), &cell).unwrap();
    let mut cl = cmdline(b"f");
    cl.builtin = true;
    let r = crate::function_call::try_call(&text(b"builtin f"), &cl, &cell).unwrap();
    assert!(r.is_none());
}

#![allow(clippy::unwrap_used)]
use alloc::vec;

use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText, ShortCStr};

use crate::error::cmd::CmdError;
use crate::parse::{BuiltinPrefix, CommandLine};
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
        prefix: BuiltinPrefix::None,
        command: ShortCStr::from_vec(command.to_vec()).unwrap(),
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
    cl.prefix = BuiltinPrefix::Builtin;
    let r = crate::function_call::try_call(&text(b"builtin f"), &cl, &cell).unwrap();
    assert!(r.is_none());
}

#[test]
fn command_prefix_bypasses_function() {
    let cell = make_cell();
    crate::script::run_script(&text(b"f() { v=hi; }"), &cell).unwrap();
    let mut cl = cmdline(b"f");
    cl.prefix = BuiltinPrefix::Command;
    let r = crate::function_call::try_call(&text(b"command f"), &cl, &cell).unwrap();
    assert!(r.is_none());
}

#[test]
fn local_variable_does_not_leak_into_the_caller() {
    let cell = make_cell();
    crate::script::run_script(&text(b"v=pre; f() { local v=1; }"), &cell).unwrap();
    let r = crate::function_call::try_call(&text(b"f"), &cmdline(b"f"), &cell).unwrap();
    assert!(r.is_some());
    assert_eq!(strings(&cell, b"v"), Some(c"pre".into()));
}

#[test]
fn failed_body_restores_the_frame() {
    let cell = make_cell();
    crate::script::run_script(&text(b"v=pre; f() { local v=1; local %x=1; }"), &cell).unwrap();
    let report = crate::function_call::try_call(&text(b"f"), &cmdline(b"f"), &cell).unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::LocalBadName { .. }
    ));
    assert_eq!(strings(&cell, b"v"), Some(c"pre".into()));
    assert!(!cell.borrow().unwrap().in_frame());
}

#[test]
fn return_in_body_restores_the_frame() {
    let cell = make_cell();
    crate::script::run_script(&text(b"v=pre; f() { local v=1; return 3; }"), &cell).unwrap();
    let r = crate::function_call::try_call(&text(b"f"), &cmdline(b"f"), &cell).unwrap();
    assert!(r.is_some());
    assert_eq!(strings(&cell, b"v"), Some(c"pre".into()));
    assert_eq!(cell.borrow().unwrap().last_status.exit_code(), 3);
}

#[test]
fn nested_calls_restore_the_inner_frame() {
    let cell = make_cell();
    crate::script::run_script(
        &text(b"v=pre; g() { local v=2; }; f() { local v=1; g; }"),
        &cell,
    )
    .unwrap();
    crate::function_call::try_call(&text(b"f"), &cmdline(b"f"), &cell).unwrap();
    assert_eq!(strings(&cell, b"v"), Some(c"pre".into()));
    assert!(!cell.borrow().unwrap().in_frame());
}

/// A `break` that escapes the body is a `LoopControl`, not a report: the frame
/// must be popped on that exit path too, so the next `local` writes into the
/// caller's frame.
#[test]
fn break_out_of_the_body_restores_the_frame() {
    let cell = make_cell();
    crate::script::run_script(
        &text(b"v=pre; f() { local v=1; break; }; for i in a b; do f; done"),
        &cell,
    )
    .unwrap();
    assert_eq!(strings(&cell, b"v"), Some(c"pre".into()));
    assert!(!cell.borrow().unwrap().in_frame());
}

fn strings(cell: &ForkCell<ShellState>, name: &[u8]) -> Option<ShortCStr> {
    let state = cell.borrow().unwrap();
    state
        .strings
        .get::<ShortCStr>(&ShortCStr::from_vec(name.to_vec()).unwrap())
        .map(|v| v.value.clone())
}

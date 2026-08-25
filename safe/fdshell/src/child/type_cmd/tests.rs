#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

use super::handle_type;

fn with_refs<R, F>(args: &[&str], state: &ShellState, f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr], &ShellState) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    f(&refs, state)
}

fn run(args: &[&str], state: &ShellState) -> i32 {
    with_refs(args, state, |refs, st| {
        handle_type(ShortCStr::from(c"type"), refs, &[], st).unwrap_or(1)
    })
}

#[test]
fn builtin_name_is_reported() {
    let state = ShellState::new();
    assert_eq!(run(&["echo"], &state), 0);
}

#[test]
fn function_and_alias_are_reported() {
    let state = ShellState::new();
    let cell = ForkCell::new(state);
    {
        let mut st = cell.borrow_mut().unwrap();
        st.functions
            .insert(ShortCStr::from(c"myfunc"), ShortCStr::from(c"true"));
        st.aliases
            .insert(ShortCStr::from(c"ll"), ShortCStr::from(c"ls -l"));
    }
    let state = cell.borrow().unwrap();
    assert_eq!(run(&["myfunc"], &state), 0);
    assert_eq!(run(&["ll"], &state), 0);
}

#[test]
fn keyword_is_reported() {
    let state = ShellState::new();
    assert_eq!(run(&["if"], &state), 0);
}

#[test]
fn fd_variable_is_reported() {
    let cell = ForkCell::new(ShellState::new());
    {
        let (fd, _wr) = sys::pipe::pipe2(0).unwrap();
        cell.borrow_mut().unwrap().fds.insert(
            ShortCStr::from(c"myfd"),
            crate::state::FdVar {
                fd,
                trace: sys::Trace::boundary(sys::Origin::Shell),
            },
        );
    }
    assert_eq!(run(&["myfd"], &cell.borrow().unwrap()), 0);
}

#[test]
fn not_found_returns_1_and_others_still_print() {
    let state = ShellState::new();
    let cell = ForkCell::new(state);
    let code = run(
        &["definitely_not_a_cmd_xyz", "echo"],
        &cell.borrow().unwrap(),
    );
    assert_eq!(code, 1);
}

#[test]
fn no_argument_is_usage_error() {
    let state = ShellState::new();
    let res = with_refs(&[], &state, |refs, st| {
        handle_type(ShortCStr::from(c"type"), refs, &[], st)
    });
    let err = match res {
        Ok(_) => panic!("expected usage error"),
        Err(e) => e,
    };
    assert!(matches!(
        err.current_context(),
        builtins::error::BuiltinError::MissingArgument("name")
    ));
}

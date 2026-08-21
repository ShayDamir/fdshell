#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use sys::fcntl::{O_DIRECTORY, O_RDONLY};
use sys::{Origin, ShortCStr, Trace};

use crate::state::{FdVar, ShellState};
use std::format;

use super::eval;

/// Build substituted (`refs`) and original (`origs`) argument views for `args`
/// and call `f`; the backing strings live for the duration of the call.
fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

fn run(args: &[&str], state: &ShellState) -> Result<i32, error_stack::Report<BuiltinError>> {
    with_refs(args, |refs, origs| eval(refs, origs, state))
}

#[test]
fn single_string_truthiness() {
    let state = ShellState::new();
    assert_eq!(run(&["x"], &state).unwrap(), 0);
    assert_eq!(run(&[""], &state).unwrap(), 1);
    // A lone operator word is a non-empty string: true (bash rule).
    assert_eq!(run(&["-f"], &state).unwrap(), 0);
}

#[test]
fn zero_operands_is_false() {
    let state = ShellState::new();
    assert_eq!(run(&[], &state).unwrap(), 1);
}

#[test]
fn string_equality() {
    let state = ShellState::new();
    assert_eq!(run(&["a", "=", "a"], &state).unwrap(), 0);
    assert_eq!(run(&["a", "=", "b"], &state).unwrap(), 1);
    assert_eq!(run(&["a", "!=", "b"], &state).unwrap(), 0);
    assert_eq!(run(&["a", "!=", "a"], &state).unwrap(), 1);
}

#[test]
fn integer_comparisons() {
    let state = ShellState::new();
    assert_eq!(run(&["1", "-eq", "1"], &state).unwrap(), 0);
    assert_eq!(run(&["1", "-ne", "2"], &state).unwrap(), 0);
    assert_eq!(run(&["1", "-lt", "2"], &state).unwrap(), 0);
    assert_eq!(run(&["2", "-le", "2"], &state).unwrap(), 0);
    assert_eq!(run(&["-5", "-gt", "-6"], &state).unwrap(), 0);
    assert_eq!(run(&["2", "-ge", "3"], &state).unwrap(), 1);
    // Equality boundaries: strictness of -lt/-gt, inclusiveness of -ge.
    assert_eq!(run(&["1", "-lt", "1"], &state).unwrap(), 1);
    assert_eq!(run(&["1", "-gt", "1"], &state).unwrap(), 1);
    assert_eq!(run(&["2", "-ge", "2"], &state).unwrap(), 0);
}

#[test]
fn file_test_rejects_unknown_unary_op() {
    let state = ShellState::new();
    let e = super::ops::file_test(b"-z", c"/tmp", None, &state).unwrap_err();
    assert!(matches!(e.current_context(), BuiltinError::Never));
}

#[test]
fn non_integer_operand_is_error() {
    let state = ShellState::new();
    assert!(matches!(
        run(&["a", "-eq", "1"], &state)
            .unwrap_err()
            .current_context(),
        BuiltinError::TestNonInteger
    ));
}

#[test]
fn malformed_expressions() {
    let state = ShellState::new();
    let cases: &[&[&str]] = &[&["a", "b", "c", "d"], &["a", "b"], &["a", "~", "b"]];
    for args in cases {
        assert!(
            matches!(
                run(args, &state).unwrap_err().current_context(),
                BuiltinError::TestUsage
            ),
            "expected TestUsage for {args:?}"
        );
    }
}

#[test]
fn file_tests_on_paths() {
    let state = ShellState::new();
    let dir = std::env::temp_dir();
    let file = dir.join(format!("fdshell-test-{}.file", std::process::id()));
    std::fs::write(&file, b"x").unwrap();
    let file_s = file.to_str().unwrap();
    assert_eq!(run(&["-f", file_s], &state).unwrap(), 0);
    assert_eq!(run(&["-d", file_s], &state).unwrap(), 1);
    assert_eq!(run(&["-e", file_s], &state).unwrap(), 0);
    assert_eq!(
        run(&["-e", "/nonexistent-fdshell-test"], &state).unwrap(),
        1
    );
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn file_tests_on_fd_vars() {
    let mut state = ShellState::new();
    let dir = std::env::temp_dir();
    let file = dir.join(format!("fdshell-test-{}.orig", std::process::id()));
    std::fs::write(&file, b"x").unwrap();
    let file_cstr = CString::new(file.to_str().unwrap()).unwrap();
    let dir_cstr = CString::new("/tmp").unwrap();
    let file_fd = sys::openat2::open(&file_cstr, O_RDONLY).unwrap();
    let dir_fd = sys::openat2::open(&dir_cstr, O_DIRECTORY).unwrap();
    state.fds.insert(
        c"f".into(),
        FdVar {
            fd: file_fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    state.fds.insert(
        c"d".into(),
        FdVar {
            fd: dir_fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );

    // Without the original argument the substituted value (an fd number) is
    // just a path: no such file exists.
    assert_eq!(run(&["-f", "0"], &state).unwrap(), 1);

    // With the original `%var` argument the fd table is consulted.
    assert_eq!(
        with_refs(&["-f", "%f"], |refs, origs| eval(refs, origs, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-d", "%d"], |refs, origs| eval(refs, origs, &state)).unwrap(),
        0
    );
    assert_eq!(
        with_refs(&["-e", "%missing"], |refs, origs| eval(refs, origs, &state)).unwrap(),
        1
    );
    std::fs::remove_file(&file).unwrap();
}

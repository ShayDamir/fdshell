#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use error_stack::Report;
use sys::{Origin, ShortCStr, Trace};

use crate::state::{FdVar, ShellState};

use super::handle_flock;
use super::parse::flock_parse;

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

fn is_invalid(e: &Report<BuiltinError>, what: &'static str) -> bool {
    matches!(e.current_context(), BuiltinError::InvalidArgument(s) if *s == what)
}

fn state_with_memfd() -> ShellState {
    let mut state = ShellState::new();
    let fd = sys::memfd::memfd_create().unwrap();
    state.fds.insert(
        c"f".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    state
}

#[test]
fn flock_defaults_to_exclusive() {
    with_refs(&["%f"], |refs, origs| {
        let cfg = flock_parse(refs, origs).unwrap();
        assert_eq!(cfg.var.as_bytes().unwrap(), b"f");
        assert_eq!(cfg.operation, sys::fcntl::LOCK_EX);
    });
}

#[test]
fn flock_flag_combinations() {
    for (args, op) in [
        (&["%f", "--shared"][..], sys::fcntl::LOCK_SH),
        (
            &["%f", "--nowait"][..],
            sys::fcntl::LOCK_EX + sys::fcntl::LOCK_NB,
        ),
        (
            &["%f", "--shared", "--nowait"][..],
            sys::fcntl::LOCK_SH + sys::fcntl::LOCK_NB,
        ),
        (&["%f", "--unlock"][..], sys::fcntl::LOCK_UN),
        (&["%f", "--wait"][..], sys::fcntl::LOCK_EX),
        (&["%f", "--shared", "--wait"][..], sys::fcntl::LOCK_SH),
    ] {
        with_refs(args, |refs, origs| {
            assert_eq!(flock_parse(refs, origs).unwrap().operation, op, "{args:?}");
        });
    }
}

#[test]
fn flock_rejects_unknown_flags() {
    with_refs(&["%f", "--nuclear"], |refs, origs| {
        let e = flock_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "flag"));
    });
}

#[test]
fn flock_rejects_unlock_combined() {
    for args in [
        &["%f", "--unlock", "--shared"][..],
        &["%f", "--unlock", "--nowait"][..],
    ] {
        with_refs(args, |refs, origs| {
            let e = flock_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "flag"), "{args:?}");
        });
    }
}

#[test]
fn flock_requires_fd_var() {
    with_refs(&[], |refs, origs| {
        let e = flock_parse(refs, origs).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("fd var")
        ));
    });
    with_refs(&["f"], |refs, origs| {
        let e = flock_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "fd var"));
    });
}

#[test]
fn flock_help() {
    with_refs(&["--help"], |refs, origs| {
        let e = flock_parse(refs, origs).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Help));
    });
}

#[test]
fn flock_handler_locks_and_unlocks_memfd() {
    let state = state_with_memfd();
    with_refs(&["%f"], |refs, origs| {
        assert_eq!(
            handle_flock(c"flock".into(), refs, origs, &state).unwrap(),
            0
        );
    });
    with_refs(&["%f", "--shared"], |refs, origs| {
        assert_eq!(
            handle_flock(c"flock".into(), refs, origs, &state).unwrap(),
            0
        );
    });
    with_refs(&["%f", "--unlock"], |refs, origs| {
        assert_eq!(
            handle_flock(c"flock".into(), refs, origs, &state).unwrap(),
            0
        );
    });
}

#[test]
fn flock_handler_nowait_on_own_lock_succeeds() {
    // A dup shares the open file description, so no self-conflict.
    let state = state_with_memfd();
    with_refs(&["%f", "--nowait"], |refs, origs| {
        assert_eq!(
            handle_flock(c"flock".into(), refs, origs, &state).unwrap(),
            0
        );
    });
}

#[test]
fn flock_handler_unset_var_is_fdvar_not_found() {
    let state = state_with_memfd();
    with_refs(&["%missing"], |refs, origs| {
        let e = handle_flock(c"flock".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

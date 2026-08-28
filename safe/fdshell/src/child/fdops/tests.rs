#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use error_stack::Report;
use sys::{Origin, ShortCStr, Trace};

use crate::state::{FdVar, ShellState};

use super::args::FtruncateConfig;
use super::parse::{fsync_parse, ftruncate_parse, lseek_parse};
use super::{handle_fsync, handle_ftruncate};

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
fn lseek_parses_offset_and_default_whence() {
    with_refs(&["%f", "10"], |refs, origs| {
        let cfg = lseek_parse(refs, origs).unwrap();
        assert_eq!(cfg.var.as_bytes().unwrap(), b"f");
        assert_eq!(cfg.offset, 10);
        assert_eq!(cfg.whence, sys::fcntl::SEEK_SET);
    });
}

#[test]
fn lseek_parses_all_whence_forms() {
    for (arg, expect) in [
        ("0", sys::fcntl::SEEK_SET),
        ("1", sys::fcntl::SEEK_CUR),
        ("2", sys::fcntl::SEEK_END),
    ] {
        with_refs(&["%f", "0", arg], |refs, origs| {
            let cfg = lseek_parse(refs, origs).unwrap();
            assert_eq!(cfg.whence, expect);
        });
    }
}

#[test]
fn lseek_allows_negative_offset() {
    with_refs(&["%f", "-3"], |refs, origs| {
        let cfg = lseek_parse(refs, origs).unwrap();
        assert_eq!(cfg.offset, -3);
    });
}

#[test]
fn lseek_missing_offset_errors() {
    with_refs(&["%f"], |refs, origs| {
        let e = lseek_parse(refs, origs).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("offset")
        ));
    });
}

#[test]
fn lseek_bad_offset_or_whence_errors() {
    for args in [&["%f", "abc"][..], &["%f", "1", "9"][..]] {
        with_refs(args, |refs, origs| {
            let e = lseek_parse(refs, origs).unwrap_err();
            assert!(
                is_invalid(&e, "offset") || is_invalid(&e, "whence"),
                "{args:?}"
            );
        });
    }
}

#[test]
fn lseek_help() {
    with_refs(&["--help"], |refs, origs| {
        let e = lseek_parse(refs, origs).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Help));
    });
}

#[test]
fn ftruncate_length_optional() {
    with_refs(&["%f"], |refs, origs| {
        match ftruncate_parse(refs, origs).unwrap() {
            FtruncateConfig { length: None, .. } => {}
            cfg => panic!("expected omitted length, got {cfg:?}"),
        }
    });
    with_refs(&["%f", "3"], |refs, origs| {
        match ftruncate_parse(refs, origs).unwrap() {
            FtruncateConfig {
                length: Some(n), ..
            } => assert_eq!(n, 3),
            _ => panic!("expected length 3"),
        }
    });
}

#[test]
fn ftruncate_length_must_be_non_negative() {
    for args in [&["%f", "-1"][..], &["%f", "x"][..]] {
        with_refs(args, |refs, origs| {
            let e = ftruncate_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "length"), "{args:?}");
        });
    }
}

#[test]
fn fsync_parses_single_var() {
    with_refs(&["%f"], |refs, origs| {
        assert_eq!(
            fsync_parse(refs, origs).unwrap().var.as_bytes().unwrap(),
            b"f"
        );
    });
}

#[test]
fn fd_var_argument_must_be_prefixed() {
    for args in [&[][..], &["f"][..], &["%%"][..]] {
        with_refs(args, |refs, origs| {
            let e = fsync_parse(refs, origs).unwrap_err();
            assert!(
                is_invalid(&e, "fd var")
                    || matches!(e.current_context(), BuiltinError::MissingArgument("fd var")),
                "{args:?}"
            );
        });
    }
}

#[test]
fn extra_arguments_error() {
    with_refs(&["%f", "%g"], |refs, origs| {
        let e = fsync_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "arg"));
    });
}

#[test]
fn unset_fd_var_is_fdvar_not_found() {
    let state = state_with_memfd();
    with_refs(&["%missing"], |refs, origs| {
        let e = handle_fsync(c"fsync".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn handlers_succeed_on_real_fd_var() {
    let state = state_with_memfd();
    with_refs(&["%f", "3"], |refs, origs| {
        assert_eq!(
            handle_ftruncate(c"ftruncate".into(), refs, origs, &state).unwrap(),
            0
        );
    });
    with_refs(&["%f"], |refs, origs| {
        assert_eq!(
            handle_fsync(c"fsync".into(), refs, origs, &state).unwrap(),
            0
        );
    });
}
